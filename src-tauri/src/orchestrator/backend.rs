use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tokio::time::{sleep, Duration, Instant};

use crate::orchestrator::embedded_postgres::DATABASE_URL;

static BACKEND: Mutex<Option<Child>> = Mutex::new(None);

fn port_in_use(port: u16) -> bool {
    std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_err()
}

pub async fn ensure_backend(app: &AppHandle) -> Result<(), String> {
    if port_in_use(3001) {
        if check_health().await {
            println!("[backend] already running and healthy");
            return Ok(());
        }
        return Err("Port 3001 is in use by another process".into());
    }

    spawn(app)?;
    wait_until_healthy(30).await
}

fn spawn(app: &AppHandle) -> Result<(), String> {
    let server_path = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join(if cfg!(windows) { "server.exe" } else { "server" });

    if !server_path.exists() {
        return Err(format!("server binary not found at {:?}", server_path));
    }

    println!("[backend] spawning {:?}", server_path);

    // Read Plaid credentials from environment at build time or fall back
    // to values set in tauri.conf.json → env section.
    let plaid_client_id = std::env::var("PLAID_CLIENT_ID")
        .unwrap_or_else(|_| "REPLACE_ME".into());
    let plaid_secret = std::env::var("PLAID_SECRET")
        .unwrap_or_else(|_| "REPLACE_ME".into());

    let child = Command::new(&server_path)
        .env("PORT", "3001")
        .env("DATABASE_URL", DATABASE_URL)
        .env("PLAID_ENV", "sandbox")
        .env("PLAID_CLIENT_ID", plaid_client_id)
        .env("PLAID_SECRET", plaid_secret)
        // Instruct NestJS not to do interactive stdin
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    println!("[backend] PID {}", child.id());
    *BACKEND.lock().unwrap() = Some(child);
    Ok(())
}

/// Hit any lightweight endpoint that confirms NestJS is accepting requests.
async fn check_health() -> bool {
    reqwest::get("http://localhost:3001/health")
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

async fn wait_until_healthy(timeout_secs: u64) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    while Instant::now() < deadline {
        if check_health().await {
            println!("[backend] healthy");
            return Ok(());
        }
        sleep(Duration::from_millis(500)).await;
    }
    Err("Backend did not become healthy in time".into())
}

pub fn shutdown() {
    if let Ok(mut guard) = BACKEND.lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            println!("[backend] killed");
        }
    }
}