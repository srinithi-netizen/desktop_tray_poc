pub mod embedded_postgres;
pub mod backend;
pub mod progress;

use tauri::AppHandle;

pub async fn run_startup(app: AppHandle) -> Result<(), String> {
    // Step 1 — initialise the DB cluster on first run (no-op after that)
    progress::emit(&app, "postgres_init", "Preparing database...");
    embedded_postgres::ensure_initdb(&app)?;

    // Step 2 — start the embedded PostgreSQL process
    progress::emit(&app, "postgres_starting", "Starting database...");
    embedded_postgres::start_postgres(&app)?;

    // Step 3 — wait until it accepts connections
    progress::emit(&app, "postgres_waiting", "Waiting for database...");
    embedded_postgres::wait_until_ready(&app, 30).await?;

    // Step 4 — create the fluxbooks database if it doesn't exist yet
    embedded_postgres::ensure_database(&app)?;

    // Step 5 — start the bundled NestJS server
    progress::emit(&app, "backend_starting", "Starting backend...");
    backend::ensure_backend(&app).await?;

    progress::emit(&app, "ready", "Ready");
    Ok(())
}

/// Called when the user quits the app.
pub fn shutdown(app: &AppHandle) {
    backend::shutdown();
    embedded_postgres::shutdown(app);
}