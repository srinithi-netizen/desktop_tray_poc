use std::process::{Command, Child, Stdio};
use std::sync::Mutex;
use once_cell::sync::Lazy;
use tokio::time::{sleep, Duration, Instant};
use tauri::AppHandle;

static BACKEND: Lazy<Mutex<Option<Child>>> = Lazy::new(|| Mutex::new(None));

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
    wait_until_healthy(15).await
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

    let child = Command::new(&server_path)
        .env("PORT", "3001")
        // Point to the embedded postgres we started above
        .env("DATABASE_URL", "postgresql://fluxbooks:fluxbooks123@127.0.0.1:5432/fluxbooks")
        .env("PLAID_ENV", "sandbox")
        .env("PLAID_CLIENT_ID", "6a3bbeeb89e19d000ef29c83")   // ← replace with real values
        .env("PLAID_SECRET", "73cb90f20796c53e1f48280a22c723")          // ← replace with real values
        // Pipe stderr so crashes are visible in Tauri logs
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    println!("[backend] PID {}", child.id());
    *BACKEND.lock().unwrap() = Some(child);
    Ok(())
}

async fn check_health() -> bool {
    reqwest::get("http://localhost:3001/plaid/link-token")
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