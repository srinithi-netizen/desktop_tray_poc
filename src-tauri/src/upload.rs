use crate::db;
use reqwest::multipart;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::fs;
use tauri::AppHandle;
use uuid::Uuid;

const SERVER_URL: &str = "http://192.168.1.81:3002";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UploadRecord {
    pub id:           String,
    pub file_name:    String,
    pub local_path:   String,
    pub file_size:    i64,
    pub status:       String,
    pub progress:     i64,
    pub total_chunks: i64,
    pub done_chunks:  i64,
    pub error_msg:    Option<String>,
    pub queued_at:    String,
    pub uploaded_at:  Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChunkRecord {
    pub id:          i64,
    pub upload_id:   String,
    pub chunk_index: i64,
    pub total:       i64,
    pub status:      String,
    pub size_bytes:  i64,
    pub sent_at:     Option<String>,
    pub error_msg:   Option<String>,
}

#[derive(Deserialize, Debug)]
struct InitResponse {
    #[serde(rename = "uploadId")]
    upload_id: String,
    #[serde(rename = "objectName")]
    object_name: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct PartResponse {
    #[serde(rename = "PartNumber")]
    part_number: i64,
    #[serde(rename = "ETag")]
    etag: String,
}

// ── Queue a file ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn queue_file(
    app: AppHandle,
    file_path: String,
    is_online: bool,
) -> Result<UploadRecord, String> {
    eprintln!("[queue_file] called with path='{}' is_online={}", file_path, is_online);

    let src = std::path::Path::new(&file_path);
    let file_name = src
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let file_size = fs::metadata(&file_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    eprintln!("[queue_file] file_name='{}' file_size={} bytes", file_name, file_size);

    let id = Uuid::new_v4().to_string();

    let uploads_dir = db::uploads_dir(&app);
    let local_filename = format!("{}_{}", id, file_name);
    let local_path = uploads_dir.join(&local_filename);
    fs::copy(&file_path, &local_path)
        .map_err(|e| format!("Failed to copy file: {}", e))?;
    let local_path_str = local_path.to_string_lossy().to_string();

    let chunk_size = 5 * 1024 * 1024_i64;
    let total_chunks = ((file_size as f64) / (chunk_size as f64)).ceil() as i64;
    let total_chunks = total_chunks.max(1);
    let queued_at = chrono::Local::now().to_rfc3339();

    let conn = db::get_conn(&app).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO uploads
            (id, file_name, local_path, file_size, status, progress,
             total_chunks, done_chunks, queued_at)
         VALUES (?1,?2,?3,?4,'pending',0,?5,0,?6)",
        params![id, file_name, local_path_str, file_size, total_chunks, queued_at],
    ).map_err(|e| e.to_string())?;

    for i in 0..total_chunks {
        let start = i * chunk_size;
        let end = ((i + 1) * chunk_size).min(file_size);
        let size = end - start;
        conn.execute(
            "INSERT OR IGNORE INTO chunks
                (upload_id, chunk_index, total, status, size_bytes)
             VALUES (?1,?2,?3,'pending',?4)",
            params![id, i, total_chunks, size],
        ).map_err(|e| e.to_string())?;
    }

    let record = UploadRecord {
        id: id.clone(),
        file_name,
        local_path: local_path_str,
        file_size,
        status: "pending".to_string(),
        progress: 0,
        total_chunks,
        done_chunks: 0,
        error_msg: None,
        queued_at,
        uploaded_at: None,
    };

    if is_online {
        let app_clone = app.clone();
        let record_clone = record.clone();
        tauri::async_runtime::spawn(async move {
            attempt_upload(&app_clone, &record_clone).await;
        });
    }

    Ok(record)
}

// ── Get all uploads ───────────────────────────────────────────────────

#[tauri::command]
pub fn get_queue(app: AppHandle) -> Result<Vec<UploadRecord>, String> {
    let conn = db::get_conn(&app).map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, file_name, local_path, file_size, status, progress,
                total_chunks, done_chunks, error_msg, queued_at, uploaded_at
         FROM uploads ORDER BY queued_at DESC"
    ).map_err(|e| e.to_string())?;

    let records: Vec<UploadRecord> = stmt.query_map([], |row| {
        Ok(UploadRecord {
            id:           row.get(0)?,
            file_name:    row.get(1)?,
            local_path:   row.get(2)?,
            file_size:    row.get(3)?,
            status:       row.get(4)?,
            progress:     row.get(5)?,
            total_chunks: row.get(6)?,
            done_chunks:  row.get(7)?,
            error_msg:    row.get(8)?,
            queued_at:    row.get(9)?,
            uploaded_at:  row.get(10)?,
        })
    }).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();

    Ok(records)
}

// ── Get chunks for a specific upload ─────────────────────────────────

#[tauri::command]
pub fn get_chunks(app: AppHandle, upload_id: String) -> Result<Vec<ChunkRecord>, String> {
    let conn = db::get_conn(&app).map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, upload_id, chunk_index, total, status, size_bytes, sent_at, error_msg
         FROM chunks WHERE upload_id = ?1 ORDER BY chunk_index ASC"
    ).map_err(|e| e.to_string())?;

    let records: Vec<ChunkRecord> = stmt.query_map(params![upload_id], |row| {
        Ok(ChunkRecord {
            id:          row.get(0)?,
            upload_id:   row.get(1)?,
            chunk_index: row.get(2)?,
            total:       row.get(3)?,
            status:      row.get(4)?,
            size_bytes:  row.get(5)?,
            sent_at:     row.get(6)?,
            error_msg:   row.get(7)?,
        })
    }).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();

    Ok(records)
}

// ── Retry pending/failed uploads ──────────────────────────────────────

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

// ── Delete upload ─────────────────────────────────────────────────────

#[tauri::command]
pub fn delete_upload(app: AppHandle, id: String) -> Result<(), String> {
    let conn = db::get_conn(&app).map_err(|e| e.to_string())?;
    let local_path: Option<String> = conn.query_row(
        "SELECT local_path FROM uploads WHERE id = ?1",
        params![id],
        |row| row.get(0),
    ).ok();

    conn.execute("DELETE FROM uploads WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    if let Some(path) = local_path {
        fs::remove_file(path).ok();
    }
    Ok(())
}

// ── Internal: upload via NestJS multipart API → MinIO ────────────────

async fn attempt_upload(app: &AppHandle, record: &UploadRecord) {
    eprintln!("[attempt_upload] START id='{}'", record.id);

    if let Ok(conn) = db::get_conn(app) {
        conn.execute(
            "UPDATE uploads SET status='uploading', error_msg=NULL WHERE id=?1",
            params![record.id],
        ).ok();
    }

    let file_bytes = match fs::read(&record.local_path) {
        Ok(b) => b,
        Err(e) => {
            update_upload_status(app, &record.id, "failed", 0, 0, Some(&e.to_string()));
            return;
        }
    };

    let client = reqwest::Client::new();
    let chunk_size = 5 * 1024 * 1024_usize;
    let all_chunks: Vec<&[u8]> = file_bytes.chunks(chunk_size).collect();
    let total_chunks = all_chunks.len();

    // ── Single chunk ──────────────────────────────────────────────────
    if total_chunks == 1 {
        let url = format!("{}/documents/upload", SERVER_URL);

        let file_part = multipart::Part::bytes(file_bytes.clone())
            .file_name(record.file_name.clone())
            .mime_str("application/octet-stream")
            .unwrap();

        let form = multipart::Form::new()
            .part("file", file_part);

        match client.post(&url).multipart(form).send().await {
            Ok(r) if r.status().is_success() => {
                let uploaded_at = chrono::Local::now().to_rfc3339();
                if let Ok(conn) = db::get_conn(app) {
                    conn.execute(
                        "UPDATE uploads SET status='completed', progress=100,
                         done_chunks=1, uploaded_at=?1 WHERE id=?2",
                        params![uploaded_at, record.id],
                    ).ok();
                    conn.execute(
                        "UPDATE chunks SET status='done', sent_at=?1 WHERE upload_id=?2",
                        params![uploaded_at, record.id],
                    ).ok();
                }
            }
            Ok(r) => {
                let err = format!("Upload failed: status={}", r.status());
                update_upload_status(app, &record.id, "failed", 0, 0, Some(&err));
            }
            Err(e) => {
                update_upload_status(app, &record.id, "failed", 0, 0, Some(&e.to_string()));
            }
        }
        return;
    }

    // ── Multiple chunks: Step 1 — Init (clientId sent HERE only) ─────
    let client_id = db::get_device_id(app);
    let init_url = format!("{}/documents/multipart/init", SERVER_URL);

    let init_body = serde_json::json!({
        "filename":    record.file_name,
        "contentType": "application/octet-stream",
        "clientId":    client_id
    });

    let init_data: InitResponse = match client.post(&init_url).json(&init_body).send().await {
        Ok(r) if r.status().is_success() => {
            match r.json::<InitResponse>().await {
                Ok(d) => d,
                Err(e) => {
                    update_upload_status(app, &record.id, "failed", 0, 0, Some(&e.to_string()));
                    return;
                }
            }
        }
        Ok(r) => {
            let err = format!("Init failed: status={} body={}", r.status(), r.text().await.unwrap_or_default());
            update_upload_status(app, &record.id, "failed", 0, 0, Some(&err));
            return;
        }
        Err(e) => {
            update_upload_status(app, &record.id, "failed", 0, 0, Some(&e.to_string()));
            return;
        }
    };

    let minio_upload_id = init_data.upload_id;
    let object_name = init_data.object_name;

    // ── Step 2: Upload parts (no clientId) ───────────────────────────
    let done_indices: Vec<i64> = {
        let conn = match db::get_conn(app) { Ok(c) => c, Err(_) => return };
        let mut stmt = match conn.prepare(
            "SELECT chunk_index FROM chunks WHERE upload_id=?1 AND status='done'"
        ) { Ok(s) => s, Err(_) => return };
        stmt.query_map(params![record.id], |row| row.get(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    };

    let mut completed_parts: Vec<PartResponse> = {
        let conn = match db::get_conn(app) { Ok(c) => c, Err(_) => return };
        let mut stmt = match conn.prepare(
            "SELECT chunk_index, etag FROM chunks WHERE upload_id=?1 AND status='done'"
        ) { Ok(s) => s, Err(_) => return };
        stmt.query_map(params![record.id], |row| {
            Ok(PartResponse {
                part_number: row.get::<_, i64>(0)? + 1,
                etag: row.get::<_, String>(1).unwrap_or_default(),
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    };

    let mut done_count = done_indices.len() as i64;
    let part_url = format!("{}/documents/multipart/part", SERVER_URL);

    for (i, chunk_data) in all_chunks.iter().enumerate() {
        let chunk_index = i as i64;
        let part_number = i as i64 + 1;

        if done_indices.contains(&chunk_index) { continue; }

        let chunk_vec = chunk_data.to_vec();
        let chunk_len = chunk_vec.len() as u64;

        if let Ok(conn) = db::get_conn(app) {
            conn.execute(
                "UPDATE chunks SET status='uploading' WHERE upload_id=?1 AND chunk_index=?2",
                params![record.id, chunk_index],
            ).ok();
        }

        let file_part = multipart::Part::bytes(chunk_vec)
            .file_name(record.file_name.clone())
            .mime_str("application/octet-stream")
            .unwrap();

        let form = multipart::Form::new()
            .text("uploadId",      minio_upload_id.clone())
            .text("objectName",    object_name.clone())
            .text("partNumber",    part_number.to_string())
            .text("contentLength", chunk_len.to_string())
            .part("file",          file_part);
        // ↑ no clientId here

        match client.post(&part_url).multipart(form).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<PartResponse>().await {
                    Ok(part) => {
                        let sent_at = chrono::Local::now().to_rfc3339();
                        done_count += 1;
                        if let Ok(conn) = db::get_conn(app) {
                            conn.execute(
                                "UPDATE chunks SET status='done', sent_at=?1, error_msg=NULL, etag=?2
                                 WHERE upload_id=?3 AND chunk_index=?4",
                                params![sent_at, part.etag.clone(), record.id, chunk_index],
                            ).ok();
                        }
                        completed_parts.push(part);
                        let progress = (done_count * 100 / total_chunks as i64) as i64;
                        update_upload_status(app, &record.id, "uploading", progress, done_count, None);
                    }
                    Err(e) => {
                        mark_chunk_failed(app, &record.id, chunk_index, &e.to_string());
                        update_upload_status(app, &record.id, "failed", 0, done_count, Some(&e.to_string()));
                        return;
                    }
                }
            }
            Ok(resp) => {
                let err = format!("Part failed: status={}", resp.status());
                mark_chunk_failed(app, &record.id, chunk_index, &err);
                update_upload_status(app, &record.id, "failed", 0, done_count, Some(&err));
                return;
            }
            Err(e) => {
                mark_chunk_failed(app, &record.id, chunk_index, &e.to_string());
                update_upload_status(app, &record.id, "failed", 0, done_count, Some(&e.to_string()));
                return;
            }
        }
    }

    // ── Step 3: Complete (no clientId) ───────────────────────────────
    completed_parts.sort_by_key(|p| p.part_number);
    let complete_url = format!("{}/documents/multipart/complete", SERVER_URL);
    let complete_body = serde_json::json!({
        "objectName":  object_name,
        "uploadId":    minio_upload_id,
        "parts":       completed_parts,
        "contentType": "application/octet-stream"
    });

    match client.post(&complete_url).json(&complete_body).send().await {
        Ok(r) if r.status().is_success() => {
            let uploaded_at = chrono::Local::now().to_rfc3339();
            if let Ok(conn) = db::get_conn(app) {
                conn.execute(
                    "UPDATE uploads SET status='completed', progress=100,
                     done_chunks=?1, uploaded_at=?2 WHERE id=?3",
                    params![total_chunks as i64, uploaded_at, record.id],
                ).ok();
            }
        }
        Ok(r) => {
            let err = format!("Complete failed: status={} body={}", r.status(), r.text().await.unwrap_or_default());
            update_upload_status(app, &record.id, "failed", 0, done_count, Some(&err));
        }
        Err(e) => {
            update_upload_status(app, &record.id, "failed", 0, done_count, Some(&e.to_string()));
        }
    }

    eprintln!("[attempt_upload] END id='{}'", record.id);
}

// ── Helpers ───────────────────────────────────────────────────────────

fn mark_chunk_failed(app: &AppHandle, upload_id: &str, chunk_index: i64, error: &str) {
    if let Ok(conn) = db::get_conn(app) {
        conn.execute(
            "UPDATE chunks SET status='failed', error_msg=?1
             WHERE upload_id=?2 AND chunk_index=?3",
            params![error, upload_id, chunk_index],
        ).ok();
    }
}

fn update_upload_status(app: &AppHandle, id: &str, status: &str, progress: i64, done_chunks: i64, error: Option<&str>) {
    if let Ok(conn) = db::get_conn(app) {
        conn.execute(
            "UPDATE uploads SET status=?1, progress=?2, done_chunks=?3, error_msg=?4 WHERE id=?5",
            params![status, progress, done_chunks, error, id],
        ).ok();
    }
}