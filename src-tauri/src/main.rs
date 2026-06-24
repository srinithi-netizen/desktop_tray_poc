#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod upload;

use std::process::Command;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent,
};

#[tauri::command]
fn get_device_id(app: tauri::AppHandle) -> String {
    db::get_device_id(&app)
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
        ])
        .setup(|app| {
            db::init_db(app.handle()).expect("Failed to init database");

            let resource_path = app.path()
                .resource_dir()
                .unwrap()
                .join("server.exe");

            if resource_path.exists() {
                Command::new(&resource_path)
                    .spawn()
                    .expect("Failed to start server.exe");
                println!("✅ server.exe started from {:?}", resource_path);
            } else {
                eprintln!("⚠️  server.exe not found at {:?}", resource_path);
            }

            let handle = app.handle();

            let open_item = MenuItem::with_id(handle, "open", "Open", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(handle, "quit", "Exit", true, None::<&str>)?;
            let menu = Menu::with_items(handle, &[&open_item, &quit_item])?;

            let icon_bytes = include_bytes!("../icons/tray-icon.png");
            let img = image::load_from_memory(icon_bytes)
                .expect("Failed to decode tray icon PNG")
                .to_rgba8();
            let (width, height) = img.dimensions();
            let tauri_image = tauri::image::Image::new_owned(img.into_raw(), width, height);

            TrayIconBuilder::new()
                .icon(tauri_image)
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(handle)?;

            app.on_menu_event(|app, event| {
                match event.id().as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::CloseRequested { api, .. },
                ..
            } = event {
                if label == "main" {
                    api.prevent_close();
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }
            }
        });
}