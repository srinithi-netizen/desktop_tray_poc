/// src-tauri/src/commands/plaid.rs
///
/// Tauri commands that proxy Plaid-related calls to the NestJS backend.
/// The backend owns all Plaid SDK calls and DB writes; Tauri is just the
/// bridge so the frontend (webview) can invoke them via invoke().

use serde::{Deserialize, Serialize};

const BASE: &str = "http://localhost:3001";

// ── shared error helper ───────────────────────────────────────────────────

fn req_err(e: reqwest::Error) -> String {
    format!("HTTP request failed: {e}")
}

// ── DTOs (mirror what your NestJS endpoints return) ───────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinkTokenResponse {
    pub link_token: String,
    pub expiration: String,
    pub request_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExchangeTokenRequest {
    pub public_token: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExchangeTokenResponse {
    pub access_token: String,
    pub item_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    pub transaction_id: String,
    pub account_id: String,
    pub amount: f64,
    pub date: String,
    pub name: String,
    pub merchant_name: Option<String>,
    pub category: Option<Vec<String>>,
    pub pending: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransactionsResponse {
    pub transactions: Vec<Transaction>,
    pub total_transactions: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AccountBalance {
    pub account_id: String,
    pub name: String,
    pub official_name: Option<String>,
    pub r#type: String,
    pub subtype: Option<String>,
    pub available: Option<f64>,
    pub current: f64,
    pub limit: Option<f64>,
    pub iso_currency_code: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BalancesResponse {
    pub accounts: Vec<AccountBalance>,
}

// ── Tauri commands ────────────────────────────────────────────────────────

/// Creates a Plaid Link token so the frontend can open the Link flow.
#[tauri::command]
pub async fn plaid_create_link_token() -> Result<LinkTokenResponse, String> {
    let client = reqwest::Client::new();
    client
        .post(format!("{BASE}/plaid/link-token"))
        .send()
        .await
        .map_err(req_err)?
        .json::<LinkTokenResponse>()
        .await
        .map_err(req_err)
}

/// Exchanges the public_token returned by Plaid Link for an access_token.
/// The backend stores the access_token in the DB and returns it.
#[tauri::command]
pub async fn plaid_exchange_token(
    public_token: String,
) -> Result<ExchangeTokenResponse, String> {
    let client = reqwest::Client::new();
    client
        .post(format!("{BASE}/plaid/exchange-token"))
        .json(&ExchangeTokenRequest { public_token })
        .send()
        .await
        .map_err(req_err)?
        .json::<ExchangeTokenResponse>()
        .await
        .map_err(req_err)
}

/// Fetches and stores the latest transactions for a linked item.
/// `item_id` is the Plaid item_id stored in your DB after exchange.
#[tauri::command]
pub async fn plaid_sync_transactions(
    item_id: String,
) -> Result<TransactionsResponse, String> {
    let client = reqwest::Client::new();
    client
        .post(format!("{BASE}/plaid/transactions/sync"))
        .json(&serde_json::json!({ "item_id": item_id }))
        .send()
        .await
        .map_err(req_err)?
        .json::<TransactionsResponse>()
        .await
        .map_err(req_err)
}

/// Returns stored transactions from the local DB (no Plaid call).
#[tauri::command]
pub async fn plaid_get_transactions(
    item_id: String,
    start_date: Option<String>,
    end_date: Option<String>,
) -> Result<TransactionsResponse, String> {
    let mut url = format!("{BASE}/plaid/transactions?item_id={item_id}");
    if let Some(s) = start_date {
        url.push_str(&format!("&start_date={s}"));
    }
    if let Some(e) = end_date {
        url.push_str(&format!("&end_date={e}"));
    }

    reqwest::get(&url)
        .await
        .map_err(req_err)?
        .json::<TransactionsResponse>()
        .await
        .map_err(req_err)
}

/// Returns live account balances for a linked item.
#[tauri::command]
pub async fn plaid_get_balances(item_id: String) -> Result<BalancesResponse, String> {
    reqwest::get(format!("{BASE}/plaid/balances?item_id={item_id}"))
        .await
        .map_err(req_err)?
        .json::<BalancesResponse>()
        .await
        .map_err(req_err)
}