use pg_embed::pg_enums::PgAuthMethod;
use pg_embed::pg_fetch::{PgFetchSettings, PG_V15};
use pg_embed::postgres::{PgEmbed, PgSettings};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Global handle — kept alive for the lifetime of the app.
static PG: Mutex<Option<PgEmbed>> = Mutex::new(None);

/// Full DATABASE_URL that the NestJS backend should use.
pub const DATABASE_URL: &str =
    "postgresql://fluxbooks:fluxbooks123@127.0.0.1:5432/fluxbooks";

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?;
    Ok(base.join("pgdata"))
}

/// Initialise, start, and create the DB. Idempotent on subsequent launches.
/// pg_embed downloads the platform binary on the very first call (~20 MB,
/// cached in the data dir so subsequent launches are instant).
pub async fn start(app: &AppHandle) -> Result<(), String> {
    let data = data_dir(app)?;
    std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;

    let settings = PgSettings {
        database_dir: data,
        port: 5432,
        user: "fluxbooks".into(),
        password: "fluxbooks123".into(),
        auth_method: PgAuthMethod::Plain,
        persistent: true,          // data survives restarts
        timeout: Some(Duration::from_secs(30)),
        migration_dir: None,
    };

    let fetch = PgFetchSettings {
        version: PG_V15,
        ..Default::default()       // auto-selects OS/arch
    };

    let mut pg = PgEmbed::new(settings, fetch)
        .await
        .map_err(|e| format!("pg_embed init failed: {e}"))?;

    pg.setup()
        .await
        .map_err(|e| format!("pg_embed setup failed: {e}"))?;

    pg.start_db()
        .await
        .map_err(|e| format!("pg_embed start failed: {e}"))?;

    // Create the application database (no-op if it already exists)
    if !pg
        .database_exists("fluxbooks")
        .await
        .map_err(|e| format!("database_exists check failed: {e}"))?
    {
        pg.create_database("fluxbooks")
            .await
            .map_err(|e| format!("create_database failed: {e}"))?;
        println!("[postgres] database 'fluxbooks' created");
    } else {
        println!("[postgres] database 'fluxbooks' already exists");
    }

    println!("[postgres] ready on port 5432");
    *PG.lock().unwrap() = Some(pg);
    Ok(())
}

pub async fn shutdown() {
    if let Ok(mut guard) = PG.lock() {
        if let Some(mut pg) = guard.take() {
            let _ = pg.stop_db().await;
            println!("[postgres] shut down");
        }
    }
}