pub mod backend;
pub mod embedded_postgres;
pub mod progress;

use tauri::AppHandle;

/// Called once from main.rs / lib.rs after the Tauri app is built.
/// Runs the full startup sequence and emits progress events to the frontend.
pub async fn run_startup(app: AppHandle) -> Result<(), String> {
    // ── Step 1 ── Download postgres binary if needed, init cluster, start ──
    progress::emit(&app, "postgres_starting", "Starting database…");
    embedded_postgres::start(&app).await?;

    // ── Step 2 ── Start the bundled NestJS server ──────────────────────────
    progress::emit(&app, "backend_starting", "Starting backend…");
    backend::ensure_backend(&app).await?;

    progress::emit(&app, "ready", "Ready");
    Ok(())
}

/// Called from the on_window_event / RunEvent::Exit handler.
pub async fn shutdown(app: &AppHandle) {
    backend::shutdown();
    embedded_postgres::shutdown().await;
}