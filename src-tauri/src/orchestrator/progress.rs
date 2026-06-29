use tauri::{AppHandle, Emitter};
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct StartupProgress {
    pub step: String,
    pub message: String,
}

pub fn emit(app: &AppHandle, step: &str, message: &str) {
    println!("[startup] {}: {}", step, message);
    let _ = app.emit("startup_progress", StartupProgress {
        step: step.to_string(),
        message: message.to_string(),
    });
}