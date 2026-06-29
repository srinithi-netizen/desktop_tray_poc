#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod upload;
mod orchestrator;

use tauri::{Manager, RunEvent};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

#[tauri::command]
fn get_device_id(app: tauri::AppHandle) -> String {
    db::get_device_id(&app)
}

#[tauri::command]
async fn open_plaid_link(_app: tauri::AppHandle) -> Result<String, String> {
    let resp = reqwest::get("http://localhost:3001/plaid/link-token")
        .await.map_err(|e| e.to_string())?;
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let token = json["link_token"].as_str()
        .ok_or("No link_token")?.to_string();
    let url = format!("http://localhost:3001/plaid/link-page?token={}", token);
    tauri_plugin_opener::open_url(&url, None::<&str>).map_err(|e| e.to_string())?;
    Ok(token)
}

#[tauri::command]
async fn get_plaid_accounts() -> Result<serde_json::Value, String> {
    let resp = reqwest::get("http://localhost:3001/plaid/accounts")
        .await.map_err(|e| e.to_string())?;
    resp.json().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_plaid_transactions() -> Result<serde_json::Value, String> {
    let resp = reqwest::get("http://localhost:3001/plaid/transactions?limit=500")
        .await.map_err(|e| e.to_string())?;
    resp.json().await.map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            upload::queue_file,
            upload::get_queue,
            upload::get_chunks,
            upload::retry_pending,
            upload::delete_upload,
            get_device_id,
            open_plaid_link,
            get_plaid_accounts,
            get_plaid_transactions,
        ])
        .setup(|app| {
            db::init_db(app.handle()).expect("Failed to init database");

            // Run orchestrator in background
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = orchestrator::run_startup(handle.clone()).await {
                    orchestrator::progress::emit(&handle, "error", &e);
                }
            });

            // Tray setup
            let handle = app.handle();
            let open_item = MenuItem::with_id(handle, "open", "Open", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(handle, "quit", "Exit", true, None::<&str>)?;
            let menu = Menu::with_items(handle, &[&open_item, &quit_item])?;

            let icon_bytes = include_bytes!("../icons/tray-icon.png");
            let img = image::load_from_memory(icon_bytes)
                .expect("Failed to decode tray icon")
                .to_rgba8();
            let (w, h) = img.dimensions();
            let icon = tauri::image::Image::new_owned(img.into_raw(), w, h);

            TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up, ..
                    } = event {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(handle)?;

            app.on_menu_event(|app, event| match event.id().as_ref() {
                "open" => {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
                "quit" => {
                    orchestrator::shutdown(&app);
                    // optionally: orchestrator::docker::stop_container();
                    app.exit(0);
                }
                _ => {}
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error building app")
        .run(|app, event| {
            if let RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::CloseRequested { api, .. }, ..
            } = event {
                if label == "main" {
                    api.prevent_close();
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.hide();
                    }
                }
            }
        });
}