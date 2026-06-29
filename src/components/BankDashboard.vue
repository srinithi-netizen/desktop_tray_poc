<template>
  <div class="dashboard">

    <!-- Connect Button -->
    <div class="connect-section">
      <button @click="connectBank" :disabled="connecting" class="connect-btn">
        <span v-if="connecting">Connecting...</span>
        <span v-else>🏦 Connect Bank Account</span>
      </button>
      <button @click="loadData" class="refresh-btn">🔄 Refresh</button>
    </div>

    <p v-if="connectError" class="error">{{ connectError }}</p>

    <!-- Accounts -->
    <section v-if="accounts.length > 0" class="section">
      <h2>Bank Accounts</h2>
      <div class="accounts-grid">
        <div v-for="acc in accounts" :key="acc.account_id" class="account-card">
          <div class="account-header">
            <span class="institution">{{ acc.institution_name }}</span>
            <span class="badge">{{ acc.subtype }}</span>
          </div>
          <div class="account-name">{{ acc.name }}</div>
          <div class="account-mask">••••{{ acc.mask }}</div>
          <div class="balances">
            <div class="balance">
              <span class="label">Current</span>
              <span class="amount">${{ formatAmount(acc.current_balance) }}</span>
            </div>
            <div class="balance">
              <span class="label">Available</span>
              <span class="amount">${{ formatAmount(acc.available_balance) }}</span>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Transactions -->
    <section v-if="transactions.length > 0" class="section">
      <h2>Recent Transactions</h2>
      <table class="tx-table">
        <thead>
          <tr>
            <th>Date</th>
            <th>Name</th>
            <th>Account</th>
            <th>Category</th>
            <th>Amount</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="tx in transactions" :key="tx.transaction_id">
            <td>{{ tx.date }}</td>
            <td>{{ tx.merchant_name || tx.name }}</td>
            <td>{{ tx.account_name }}</td>
            <td>{{ tx.category || '—' }}</td>
            <td :class="tx.amount < 0 ? 'credit' : 'debit'">
              {{ tx.amount < 0 ? '+' : '-' }}${{ formatAmount(Math.abs(tx.amount)) }}
            </td>
          </tr>
        </tbody>
      </table>
    </section>

    <div v-if="accounts.length === 0 && transactions.length === 0 && !connecting" class="empty">
      <p>No bank accounts connected yet.</p>
      <p style="color:#94a3b8;font-size:13px">Click "Connect Bank Account" to get started.</p>
    </div>

  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const accounts = ref([]);
const transactions = ref([]);
const connecting = ref(false);
const connectError = ref('');

function formatAmount(val) {
  if (val == null) return '0.00';
  return parseFloat(val).toFixed(2);
}

async function loadData() {
  try {
    const [accs, txs] = await Promise.all([
      invoke('get_plaid_accounts'),
      invoke('get_plaid_transactions'),
    ]);
    accounts.value = Array.isArray(accs) ? accs : [];
    transactions.value = Array.isArray(txs) ? txs : [];
  } catch (e) {
    console.error('Failed to load data:', e);
  }
}

async function connectBank() {
  connecting.value = true;
  connectError.value = '';
  try {
    await invoke('open_plaid_link');
    // Poll for new data after user completes Plaid flow
    setTimeout(loadData, 5000);
    setTimeout(loadData, 10000);
  } catch (e) {
    connectError.value = 'Failed to open Plaid: ' + e;
  } finally {
    connecting.value = false;
  }
}

onMounted(loadData);
</script>

<style scoped>
.dashboard { padding: 24px; font-family: sans-serif; max-width: 900px; margin: 0 auto; }

.connect-section { display: flex; gap: 12px; margin-bottom: 24px; }

.connect-btn {
  background: #2563eb; color: white; border: none; border-radius: 8px;
  padding: 10px 24px; font-size: 15px; font-weight: 600; cursor: pointer;
}
.connect-btn:hover:not(:disabled) { background: #1d4ed8; }
.connect-btn:disabled { opacity: 0.6; cursor: not-allowed; }

.refresh-btn {
  background: #f1f5f9; color: #334155; border: 1px solid #e2e8f0;
  border-radius: 8px; padding: 10px 18px; font-size: 15px; cursor: pointer;
}
.refresh-btn:hover { background: #e2e8f0; }

.section { margin-bottom: 32px; }
.section h2 { font-size: 18px; font-weight: 700; color: #1e293b; margin-bottom: 16px; }

.accounts-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 16px; }

.account-card {
  background: white; border: 1px solid #e2e8f0; border-radius: 12px;
  padding: 16px; box-shadow: 0 1px 3px rgba(0,0,0,0.05);
}
.account-header { display: flex; justify-content: space-between; margin-bottom: 8px; }
.institution { font-size: 12px; color: #64748b; font-weight: 600; text-transform: uppercase; }
.badge { background: #eff6ff; color: #2563eb; font-size: 11px; padding: 2px 8px; border-radius: 99px; }
.account-name { font-size: 16px; font-weight: 600; color: #1e293b; }
.account-mask { font-size: 13px; color: #94a3b8; margin-bottom: 12px; }
.balances { display: flex; gap: 16px; }
.balance { display: flex; flex-direction: column; }
.label { font-size: 11px; color: #94a3b8; text-transform: uppercase; }
.amount { font-size: 18px; font-weight: 700; color: #1e293b; }

.tx-table { width: 100%; border-collapse: collapse; font-size: 14px; }
.tx-table th { text-align: left; padding: 10px 12px; background: #f8fafc; color: #64748b; font-size: 12px; text-transform: uppercase; border-bottom: 1px solid #e2e8f0; }
.tx-table td { padding: 10px 12px; border-bottom: 1px solid #f1f5f9; color: #334155; }
.tx-table tr:hover td { background: #f8fafc; }
.debit { color: #ef4444; font-weight: 600; }
.credit { color: #16a34a; font-weight: 600; }

.empty { text-align: center; padding: 60px 20px; color: #64748b; }
.error { color: #ef4444; font-size: 14px; margin-bottom: 12px; }
</style>