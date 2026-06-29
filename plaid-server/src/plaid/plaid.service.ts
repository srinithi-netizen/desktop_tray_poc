import { Injectable, Inject } from '@nestjs/common';
import { ConfigService } from '@nestjs/config';
import {
  Configuration,
  PlaidApi,
  PlaidEnvironments,
  Products,
  CountryCode,
} from 'plaid';
import { Pool } from 'pg';

@Injectable()
export class PlaidService {
  private plaidClient: PlaidApi;

  constructor(
    private config: ConfigService,
    @Inject('DATABASE_POOL') private db: Pool,
  ) {
    // PlaidEnvironments.sandbox = 'https://sandbox.plaid.com' — NO localhost
    const plaidConfig = new Configuration({
      basePath: PlaidEnvironments[this.config.get('PLAID_ENV') || 'sandbox'],
      baseOptions: {
        headers: {
          'PLAID-CLIENT-ID': this.config.get('PLAID_CLIENT_ID'),
          'PLAID-SECRET': this.config.get('PLAID_SECRET'),
        },
      },
    });

    this.plaidClient = new PlaidApi(plaidConfig);
  }

  async createLinkToken(): Promise<string> {
    const response = await this.plaidClient.linkTokenCreate({
      user: { client_user_id: 'fluxbooks-user-1' },
      client_name: 'FluxBooks',
      products: [Products.Transactions],
      country_codes: [CountryCode.Us],
      language: 'en',
    });
    return response.data.link_token;
  }

  // Replace transactionsGet with syncTransactions (newer Plaid API)
async exchangeAndSync(publicToken: string) {
    const exchangeRes = await this.plaidClient.itemPublicTokenExchange({
      public_token: publicToken,
    });

    const accessToken = exchangeRes.data.access_token;
    const itemId = exchangeRes.data.item_id;

    const itemRes = await this.plaidClient.itemGet({ access_token: accessToken });
    const institutionId = itemRes.data.item.institution_id;
    let institutionName = 'Unknown Bank';

    if (institutionId) {
      const instRes = await this.plaidClient.institutionsGetById({
        institution_id: institutionId,
        country_codes: [CountryCode.Us],
      });
      institutionName = instRes.data.institution.name;
    }

    await this.db.query(
      `INSERT INTO plaid_items (item_id, access_token, institution_id, institution_name)
       VALUES ($1, $2, $3, $4)
       ON CONFLICT (item_id) DO UPDATE SET access_token = $2`,
      [itemId, accessToken, institutionId, institutionName],
    );

    const accountsRes = await this.plaidClient.accountsGet({ access_token: accessToken });
    const accounts = accountsRes.data.accounts;

    for (const acc of accounts) {
      await this.db.query(
        `INSERT INTO bank_accounts
          (item_id, account_id, name, official_name, type, subtype, mask, current_balance, available_balance, currency_code)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
         ON CONFLICT (account_id) DO UPDATE SET current_balance = $8, available_balance = $9`,
        [
          itemId, acc.account_id, acc.name, acc.official_name,
          acc.type, acc.subtype, acc.mask,
          acc.balances.current, acc.balances.available,
          acc.balances.iso_currency_code || 'USD',
        ],
      );
    }

    // Use transactionsSync instead of transactionsGet
    let cursor: string | undefined = undefined;
    let allTransactions: any[] = [];
    let hasMore = true;

    while (hasMore) {
      const syncRes = await this.plaidClient.transactionsSync({
        access_token: accessToken,
        cursor: cursor,
      });

      allTransactions = allTransactions.concat(syncRes.data.added);
      hasMore = syncRes.data.has_more;
      cursor = syncRes.data.next_cursor;
    }

    for (const tx of allTransactions) {
      await this.db.query(
        `INSERT INTO transactions
          (account_id, transaction_id, name, amount, date, category, merchant_name, payment_channel, pending, currency_code)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
         ON CONFLICT (transaction_id) DO NOTHING`,
        [
          tx.account_id,
          tx.transaction_id,
          tx.name,
          tx.amount,
          tx.date,
          tx.personal_finance_category?.primary || null,
          tx.merchant_name,
          tx.payment_channel,
          tx.pending,
          tx.iso_currency_code || 'USD',
        ],
      );
    }

    return {
      accountsCount: accounts.length,
      transactionsCount: allTransactions.length,
    };
  }

  async getAccounts() {
    const res = await this.db.query(`
      SELECT ba.*, pi.institution_name
      FROM bank_accounts ba
      JOIN plaid_items pi ON ba.item_id = pi.item_id
      ORDER BY ba.created_at DESC
    `);
    return res.rows;
  }

  async getTransactions(limit = 100) {
    const res = await this.db.query(
      `SELECT t.*, ba.name as account_name
       FROM transactions t
       JOIN bank_accounts ba ON t.account_id = ba.account_id
       ORDER BY t.date DESC
       LIMIT $1`,
      [limit],
    );
    return res.rows;
  }
}