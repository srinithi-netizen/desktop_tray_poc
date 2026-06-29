use std::fs;
use std::path::PathBuf;
use std::process::{Command, Child, Stdio};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};  // Manager trait needed for .path()

static POSTGRES_PROCESS: Mutex<Option<Child>> = Mutex::new(None);

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

fn pg_bin(dir: &PathBuf, name: &str) -> PathBuf {
    if cfg!(windows) {
        dir.join("bin").join(format!("{}.exe", name))
    } else {
        dir.join("bin").join(name)
    }
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?;
    Ok(base.join("pgdata"))
}

fn socket_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?;
    Ok(base.join("pgsocket"))
}

pub fn ensure_initdb(app: &AppHandle) -> Result<(), String> {
    let data = data_dir(app)?;
    let bin = postgres_bin_dir(app)?;

    if data.join("PG_VERSION").exists() {
        println!("[postgres] already initialised");
        return Ok(());
    }

    fs::create_dir_all(&data).map_err(|e| e.to_string())?;
    println!("[postgres] running initdb at {:?}", data);

    let status = Command::new(pg_bin(&bin, "initdb"))
        .args([
            "-D", data.to_str().unwrap(),
            "--username=fluxbooks",
            "--auth=trust",
            "--encoding=UTF8",
        ])
        .env("LD_LIBRARY_PATH", bin.join("lib").to_str().unwrap())
        .status()
        .map_err(|e| format!("initdb failed: {e}"))?;

    if !status.success() {
        return Err("initdb exited with non-zero status".into());
    }

    println!("[postgres] initdb complete");
    Ok(())
}

pub fn start_postgres(app: &AppHandle) -> Result<(), String> {
    let data = data_dir(app)?;
    let bin = postgres_bin_dir(app)?;
    let socket = socket_dir(app)?;
    fs::create_dir_all(&socket).map_err(|e| e.to_string())?;

    println!("[postgres] starting embedded postgres");

    let child = Command::new(pg_bin(&bin, "postgres"))
        .args([
            "-D", data.to_str().unwrap(),
            "-p", "5432",
            "-k", socket.to_str().unwrap(),
            "-h", "127.0.0.1",
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

pub fn ensure_database(app: &AppHandle) -> Result<(), String> {
    let bin = postgres_bin_dir(app)?;
    // Ignore error — database may already exist
    let _ = Command::new(pg_bin(&bin, "createdb"))
        .args(["-h", "127.0.0.1", "-p", "5432", "-U", "fluxbooks", "fluxbooks"])
        .env("LD_LIBRARY_PATH", bin.join("lib").to_str().unwrap())
        .output();
    Ok(())
}

pub async fn wait_until_ready(app: &AppHandle, timeout_secs: u64) -> Result<(), String> {
    let bin = postgres_bin_dir(app)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    while std::time::Instant::now() < deadline {
        let ok = Command::new(pg_bin(&bin, "pg_isready"))
            .args(["-h", "127.0.0.1", "-p", "5432", "-U", "fluxbooks"])
            .env("LD_LIBRARY_PATH", bin.join("lib").to_str().unwrap())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if ok {
            println!("[postgres] ready");
            return Ok(());
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    Err("PostgreSQL did not become ready in time".into())
}

pub fn shutdown(app: &AppHandle) {
    if let (Ok(bin), Ok(data)) = (postgres_bin_dir(app), data_dir(app)) {
        let _ = Command::new(pg_bin(&bin, "pg_ctl"))
            .args(["-D", data.to_str().unwrap(), "stop", "-m", "fast"])
            .env("LD_LIBRARY_PATH", bin.join("lib").to_str().unwrap())
            .status();
    }
    if let Ok(mut guard) = POSTGRES_PROCESS.lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
        }
    }
    println!("[postgres] shut down");
}