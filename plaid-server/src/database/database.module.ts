import { Global, Module } from '@nestjs/common';
import { Pool } from 'pg';
import * as dotenv from 'dotenv';
dotenv.config();  // <-- load .env before anything else

const databaseProvider = {
  provide: 'DATABASE_POOL',
  useFactory: async (): Promise<Pool> => {
    console.log('DATABASE_URL:', process.env.DATABASE_URL); // <-- verify it loads

    const pool = new Pool({
      connectionString: process.env.DATABASE_URL,
    });

    await pool.query(`
      CREATE TABLE IF NOT EXISTS plaid_items (
        id SERIAL PRIMARY KEY,
        item_id TEXT UNIQUE NOT NULL,
        access_token TEXT NOT NULL,
        institution_id TEXT,
        institution_name TEXT,
        created_at TIMESTAMPTZ DEFAULT NOW()
      );
      CREATE TABLE IF NOT EXISTS bank_accounts (
        id SERIAL PRIMARY KEY,
        item_id TEXT REFERENCES plaid_items(item_id),
        account_id TEXT UNIQUE NOT NULL,
        name TEXT,
        official_name TEXT,
        type TEXT,
        subtype TEXT,
        mask TEXT,
        current_balance NUMERIC(12,2),
        available_balance NUMERIC(12,2),
        currency_code TEXT DEFAULT 'USD',
        created_at TIMESTAMPTZ DEFAULT NOW()
      );
      CREATE TABLE IF NOT EXISTS transactions (
        id SERIAL PRIMARY KEY,
        account_id TEXT REFERENCES bank_accounts(account_id),
        transaction_id TEXT UNIQUE NOT NULL,
        name TEXT,
        amount NUMERIC(12,2),
        date DATE,
        category TEXT,
        merchant_name TEXT,
        payment_channel TEXT,
        pending BOOLEAN DEFAULT false,
        currency_code TEXT DEFAULT 'USD',
        created_at TIMESTAMPTZ DEFAULT NOW()
      );
    `);

    console.log('✅ PostgreSQL tables ready');
    return pool;
  },
};

@Global()
@Module({
  providers: [databaseProvider],
  exports: [databaseProvider],
})
export class DatabaseModule {}