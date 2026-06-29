/// src-tauri/src/lib.rs
///
/// Tauri application entry point.
/// Wire the orchestrator startup, shutdown, and Plaid commands here.

mod orchestrator;
mod commands;   // your existing commands module

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        // ── Register all Plaid commands ─────────────────────────────────
        .invoke_handler(tauri::generate_handler![
            commands::plaid::plaid_create_link_token,
            commands::plaid::plaid_exchange_token,
            commands::plaid::plaid_sync_transactions,
            commands::plaid::plaid_get_transactions,
            commands::plaid::plaid_get_balances,
            // ... add your other commands here
        ])
        // ── Run startup sequence once the app window is ready ───────────
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = orchestrator::run_startup(handle.clone()).await {
                    eprintln!("[startup] FATAL: {e}");
                    // Emit a failure event so the frontend can show an error
                    let _ = handle.emit("startup_error", e);
                }
            });
            Ok(())
        })
        // ── Clean shutdown when the last window closes ──────────────────
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                let handle = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    orchestrator::shutdown(&handle).await;
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}