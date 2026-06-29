import { Controller, Get, Post, Body, Query, Res } from '@nestjs/common';
import { Response } from 'express';
import { PlaidService } from './plaid.service';

@Controller('plaid')
export class PlaidController {
  constructor(private readonly plaidService: PlaidService) {}

  @Get('link-token')
  async getLinkToken() {
    const linkToken = await this.plaidService.createLinkToken();
    return { link_token: linkToken };
  }

 @Post('exchange-token')
async exchangeToken(@Body('public_token') publicToken: string) {
  const result = await this.plaidService.exchangeAndSync(publicToken);
    console.log('exchangeAndSync result:', result);  // <-- add this

  
  return {
    success: true,
    accountsCount: result.accountsCount,
    transactionsCount: result.transactionsCount,
  };
}

  @Get('accounts')
  async getAccounts() {
    return this.plaidService.getAccounts();
  }

  @Get('transactions')
  async getTransactions(@Query('limit') limit?: string) {
    return this.plaidService.getTransactions(limit ? parseInt(limit) : 100);
  }

  // This page is opened in the user's default browser by Tauri
  // It loads Plaid Link UI, then POSTs the public_token back to localhost:3001
  @Get('link-page')
  getLinkPage(@Query('token') token: string, @Res() res: Response) {
    res.send(`
      <!DOCTYPE html>
      <html>
      <head>
        <title>Connect Bank - FluxBooks</title>
        <style>
          body { font-family: sans-serif; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: #f8fafc; }
          .msg { text-align: center; padding: 40px; }
          .spinner { width: 40px; height: 40px; border: 4px solid #e2e8f0; border-top-color: #2563eb; border-radius: 50%; animation: spin 0.8s linear infinite; margin: 0 auto 16px; }
          @keyframes spin { to { transform: rotate(360deg); } }
          h2 { margin: 0 0 8px; }
        </style>
      </head>
      <body>
        <div id="msg" class="msg">
          <div class="spinner"></div>
          <p>Opening bank connection...</p>
        </div>
        <script src="https://cdn.plaid.com/link/v2/stable/link-initialize.js"></script>
        <script>
          const handler = Plaid.create({
            token: '${token}',
            onSuccess: async (public_token, metadata) => {
              document.getElementById('msg').innerHTML = '<div class="spinner"></div><p>Syncing your accounts...</p>';
              try {
                // POST goes to NestJS server — NOT to Plaid directly
                const res = await fetch('http://localhost:3001/plaid/exchange-token', {
                  method: 'POST',
                  headers: { 'Content-Type': 'application/json' },
                  body: JSON.stringify({ public_token })
                });
                const data = await res.json();
                document.getElementById('msg').innerHTML =
                  '<h2 style="color:#16a34a">✅ Bank Connected!</h2>' +
                  '<p>' + data.accountsCount + ' accounts and ' + data.transactionsCount + ' transactions synced.</p>' +
                  '<p style="color:#64748b;font-size:13px">You can close this window and return to FluxBooks.</p>';
              } catch(e) {
                document.getElementById('msg').innerHTML = '<h2 style="color:#ef4444">❌ Sync failed</h2><p>' + e.message + '</p>';
              }
            },
            onExit: (err) => {
              if (err) {
                document.getElementById('msg').innerHTML = '<h2 style="color:#ef4444">❌ Error</h2><p>' + (err.display_message || err.error_message) + '</p>';
              } else {
                window.close();
              }
            }
          });
          handler.open();
        </script>
      </body>
      </html>
    `);
  }
}