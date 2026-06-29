use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Child, Stdio};
use std::sync::Mutex;
use once_cell::sync::Lazy;
use tauri::AppHandle;
use tokio::time::{sleep, Duration, Instant};

static POSTGRES_PROCESS: Lazy<Mutex<Option<Child>>> = Lazy::new(|| Mutex::new(None));

/// Returns the data directory where PostgreSQL will store its files.
/// Uses the app's local data directory so it persists across launches.
fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?;
    Ok(base.join("pgdata"))
}

/// Returns the path to the bundled postgres binary.
fn postgres_bin_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let resources = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?;
 
    let folder = if cfg!(target_os = "windows") {
        "postgres-windows"
    } else if cfg!(target_os = "macos") {
        "postgres-macos"
    } else {
        "postgres-linux"
    };
 
    Ok(resources.join(folder))
}
 
/// On Windows, binaries have a .exe extension.
fn pg_bin(dir: &PathBuf, name: &str) -> PathBuf {
    if cfg!(windows) {
        dir.join("bin").join(format!("{}.exe", name))
    } else {
        dir.join("bin").join(name)
    }
}
 

/// Returns the socket directory (we use TCP instead, but PG needs a socket dir).
fn socket_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?;
    Ok(base.join("pgsocket"))
}

/// One-time: initialise the database cluster if it doesn't exist yet.
pub fn ensure_initdb(app: &AppHandle) -> Result<(), String> {
    let data = data_dir(app)?;
    let bin = postgres_bin_dir(app)?;

    // Already initialised — nothing to do.
    if data.join("PG_VERSION").exists() {
        println!("[postgres] data directory already initialised");
        return Ok(());
    }

    fs::create_dir_all(&data).map_err(|e| e.to_string())?;

    println!("[postgres] running initdb at {:?}", data);

    let initdb = bin.join("bin").join("initdb");
    let status = Command::new(&initdb)
        .args([
            "-D", data.to_str().unwrap(),
            "--username=fluxbooks",
            "--auth=trust",
            "--encoding=UTF8",
        ])
        .env("LD_LIBRARY_PATH", bin.join("lib").to_str().unwrap())
        .status()
        .map_err(|e| format!("initdb failed to launch: {e}"))?;

    if !status.success() {
        return Err("initdb exited with non-zero status".into());
    }

    println!("[postgres] initdb complete");
    Ok(())
}

/// Start the embedded PostgreSQL server.
pub fn start_postgres(app: &AppHandle) -> Result<(), String> {
    let data = data_dir(app)?;
    let bin = postgres_bin_dir(app)?;
    let socket = socket_dir(app)?;
    fs::create_dir_all(&socket).map_err(|e| e.to_string())?;

    let postgres_bin = bin.join("bin").join("postgres");

    println!("[postgres] starting embedded postgres from {:?}", postgres_bin);

    let child = Command::new(&postgres_bin)
        .args([
            "-D", data.to_str().unwrap(),
            "-p", "5432",
            "-k", socket.to_str().unwrap(),  // Unix socket dir
            "-h", "127.0.0.1",               // only listen locally
        ])
        .env("LD_LIBRARY_PATH", bin.join("lib").to_str().unwrap())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn postgres: {e}"))?;

    println!("[postgres] PID {}", child.id());
    *POSTGRES_PROCESS.lock().unwrap() = Some(child);
    Ok(())
}

/// Ensure the `fluxbooks` database exists inside the running cluster.
pub fn ensure_database(app: &AppHandle) -> Result<(), String> {
    let bin = postgres_bin_dir(app)?;
    let createdb = bin.join("bin").join("createdb");

    // createdb will fail if it already exists — that's fine, ignore the error.
    let _ = Command::new(&createdb)
        .args([
            "-h", "127.0.0.1",
            "-p", "5432",
            "-U", "fluxbooks",
            "fluxbooks",
        ])
        .env("LD_LIBRARY_PATH", bin.join("lib").to_str().unwrap())
        .output();

    Ok(())
}

/// Poll pg_isready until PostgreSQL accepts connections (or timeout).
pub async fn wait_until_ready(app: &AppHandle, timeout_secs: u64) -> Result<(), String> {
    let bin = postgres_bin_dir(app)?;
    let pg_isready = bin.join("bin").join("pg_isready");
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    while Instant::now() < deadline {
        let ok = Command::new(&pg_isready)
            .args(["-h", "127.0.0.1", "-p", "5432", "-U", "fluxbooks"])
            .env("LD_LIBRARY_PATH", bin.join("lib").to_str().unwrap())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if ok {
            println!("[postgres] ready");
            return Ok(());
        }

        sleep(Duration::from_millis(500)).await;
    }

    Err("PostgreSQL did not become ready in time".into())
}

/// Gracefully stop the embedded PostgreSQL server on app quit.
pub fn shutdown(app: &AppHandle) {
    let bin = match postgres_bin_dir(app) {
        Ok(b) => b,
        Err(_) => return,
    };
    let data = match data_dir(app) {
        Ok(d) => d,
        Err(_) => return,
    };

    // Use pg_ctl to do a fast shutdown
    let _ = Command::new(bin.join("bin").join("pg_ctl"))
        .args([
            "stop",
            "-D", data.to_str().unwrap(),
            "-m", "fast",
        ])
        .env("LD_LIBRARY_PATH", bin.join("lib").to_str().unwrap())
        .status();

    // Also kill the child process handle just in case
    if let Ok(mut guard) = POSTGRES_PROCESS.lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
        }
    }

    println!("[postgres] shut down");
}