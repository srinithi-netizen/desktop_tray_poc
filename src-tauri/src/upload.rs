use crate::db;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::fs;
use tauri::AppHandle;
use uuid::Uuid;

// This is what we send back to Vue for each upload record
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UploadRecord {
    pub id: String,
    pub file_name: String,
    pub local_path: String,
    pub file_size: i64,
    pub status: String,    // "pending" | "uploading" | "completed" | "failed"
    pub progress: i64,     // 0-100
    pub error_msg: Option<String>,
    pub queued_at: String,
    pub uploaded_at: Option<String>,
}

// Called by Vue when user picks a file
// 1. Copies file to secure local folder
// 2. Saves record to SQLite
// 3. Tries to upload if online
#[tauri::command]
pub async fn queue_file(
    app: AppHandle,
    file_path: String,
    is_online: bool,
) -> Result<UploadRecord, String> {
    let src = std::path::Path::new(&file_path);

    // Get original filename
    let file_name = src
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Get file size
    let file_size = fs::metadata(&file_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    // Generate unique ID for this upload
    let id = Uuid::new_v4().to_string();

    // Copy file to secure local folder
    // This protects the file even if original is deleted
    let uploads_dir = db::uploads_dir(&app);
    let local_filename = format!("{}_{}", id, file_name);
    let local_path = uploads_dir.join(&local_filename);

    fs::copy(&file_path, &local_path)
        .map_err(|e| format!("Failed to copy file: {}", e))?;

    let local_path_str = local_path.to_string_lossy().to_string();
    let queued_at = chrono::Local::now().to_rfc3339();

    // Save to SQLite
    let conn = db::get_conn(&app).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO uploads (id, file_name, local_path, file_size, status, progress, queued_at)
         VALUES (?1, ?2, ?3, ?4, 'pending', 0, ?5)",
        params![id, file_name, local_path_str, file_size, queued_at],
    ).map_err(|e| e.to_string())?;

    let record = UploadRecord {
        id: id.clone(),
        file_name,
        local_path: local_path_str,
        file_size,
        status: "pending".to_string(),
        progress: 0,
        error_msg: None,
        queued_at,
        uploaded_at: None,
    };

    // If online, attempt upload immediately in background
    if is_online {
        let app_clone = app.clone();
        let record_clone = record.clone();
        tauri::async_runtime::spawn(async move {
            attempt_upload(&app_clone, &record_clone).await;
        });
    }

    Ok(record)
}

// Returns all uploads from SQLite to show in UI
#[tauri::command]
pub fn get_queue(app: AppHandle) -> Result<Vec<UploadRecord>, String> {
    let conn = db::get_conn(&app).map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, file_name, local_path, file_size, status, progress,
                error_msg, queued_at, uploaded_at
         FROM uploads ORDER BY queued_at DESC"
    ).map_err(|e| e.to_string())?;

    let records = stmt.query_map([], |row| {
        Ok(UploadRecord {
            id:          row.get(0)?,
            file_name:   row.get(1)?,
            local_path:  row.get(2)?,
            file_size:   row.get(3)?,
            status:      row.get(4)?,
            progress:    row.get(5)?,
            error_msg:   row.get(6)?,
            queued_at:   row.get(7)?,
            uploaded_at: row.get(8)?,
        })
    }).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();

    Ok(records)
}

// Called by Vue when internet comes back — retries all pending/failed uploads
#[tauri::command]
pub async fn retry_pending(app: AppHandle) -> Result<(), String> {
    let records = get_queue(app.clone())?;
    let pending: Vec<UploadRecord> = records
        .into_iter()
        .filter(|r| r.status == "pending" || r.status == "failed")
        .collect();

    for record in pending {
        let app_clone = app.clone();
        let record_clone = record.clone();
        tauri::async_runtime::spawn(async move {
            attempt_upload(&app_clone, &record_clone).await;
        });
    }

    Ok(())
}

// Delete an upload record and its local file
#[tauri::command]
pub fn delete_upload(app: AppHandle, id: String) -> Result<(), String> {
    let conn = db::get_conn(&app).map_err(|e| e.to_string())?;

    // Get local path first so we can delete the file
    let local_path: Option<String> = conn.query_row(
        "SELECT local_path FROM uploads WHERE id = ?1",
        params![id],
        |row| row.get(0),
    ).ok();

    // Delete from database
    conn.execute("DELETE FROM uploads WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    // Delete local file
    if let Some(path) = local_path {
        fs::remove_file(path).ok();
    }

    Ok(())
}

// Internal: actually uploads the file to server
// Updates progress in SQLite as it goes
// Vue polls get_queue() to see progress updates
async fn attempt_upload(app: &AppHandle, record: &UploadRecord) {
    update_status(app, &record.id, "uploading", 0, None);

    let file_bytes = match fs::read(&record.local_path) {
        Ok(b) => b,
        Err(e) => {
            update_status(app, &record.id, "failed", 0, Some(&e.to_string()));
            return;
        }
    };

    let chunk_size = 512 * 1024;
    let chunks: Vec<&[u8]> = file_bytes.chunks(chunk_size).collect();
    let total_chunks = chunks.len();

    let client = reqwest::Client::new();

    for (i, chunk) in chunks.iter().enumerate() {
        // ── Using httpbin.org for POC testing ──
        // Replace this URL with your real server later
        let url = "https://httpbin.org/post".to_string();

        let result = client
            .post(&url)
            .header("X-File-Name", &record.file_name)
            .header("X-Chunk-Index", i.to_string())
            .header("X-Total-Chunks", total_chunks.to_string())
            .header("X-Upload-Id", &record.id)
            .body(chunk.to_vec())
            .send()
            .await;

        match result {
            Ok(resp) if resp.status().is_success() => {
                let progress = ((i + 1) * 100 / total_chunks) as i64;
                update_status(app, &record.id, "uploading", progress, None);
            }
            Ok(resp) => {
                let err = format!("Server error: {}", resp.status());
                update_status(app, &record.id, "failed", 0, Some(&err));
                return;
            }
            Err(e) => {
                update_status(app, &record.id, "failed", 0, Some(&e.to_string()));
                return;
            }
        }
    }

    let uploaded_at = chrono::Local::now().to_rfc3339();
    let conn = match db::get_conn(app) {
        Ok(c) => c,
        Err(_) => return,
    };
    conn.execute(
        "UPDATE uploads SET status='completed', progress=100, uploaded_at=?1 WHERE id=?2",
        params![uploaded_at, record.id],
    ).ok();
}

// Helper to update status in SQLite
fn update_status(app: &AppHandle, id: &str, status: &str, progress: i64, error: Option<&str>) {
    if let Ok(conn) = db::get_conn(app) {
        conn.execute(
            "UPDATE uploads SET status=?1, progress=?2, error_msg=?3 WHERE id=?4",
            params![status, progress, error, id],
        ).ok();
    }
}