use rusqlite::Connection;
use std::fs;
use tauri::{AppHandle, Manager};  // ← add Manager here


// Returns the path to our secure app data folder
pub fn app_data_dir(app: &AppHandle) -> std::path::PathBuf {
    let base = app
        .path()
        .app_data_dir()
        .expect("No app data dir");
    base
}

// Returns the path where we store uploaded files securely
pub fn uploads_dir(app: &AppHandle) -> std::path::PathBuf {
    let dir = app_data_dir(app).join("uploads");
    fs::create_dir_all(&dir).ok();
    dir
}

// Returns connection to our SQLite database
pub fn get_conn(app: &AppHandle) -> rusqlite::Result<Connection> {
    let db_path = app_data_dir(app).join("fluxbooks.db");
    Connection::open(db_path)
}

// Called once on app start — creates the uploads table if it doesn't exist
pub fn init_db(app: &AppHandle) -> rusqlite::Result<()> {
    let data_dir = app_data_dir(app);
    fs::create_dir_all(&data_dir).ok();

    let conn = get_conn(app)?;
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS uploads (
            id          TEXT PRIMARY KEY,
            file_name   TEXT NOT NULL,
            local_path  TEXT NOT NULL,
            file_size   INTEGER NOT NULL,
            status      TEXT NOT NULL DEFAULT 'pending',
            progress    INTEGER NOT NULL DEFAULT 0,
            error_msg   TEXT,
            queued_at   TEXT NOT NULL,
            uploaded_at TEXT
        );
    ")?;
    Ok(())
}