use tauri::{tray::TrayIconBuilder, menu::{Menu, MenuItem}, image::Image, Manager};
mod commands;
mod config_file;
mod events;
use commands::*;
use downloader::download_data_factory::DownloadDataFactory;
use downloader::download_manager::DownloadManager;
mod api_error;
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Run async reconstruction synchronously during app initialization
            let download_manager = tauri::async_runtime::block_on(async move {
                let client = reqwest::Client::new();
                let poll_every_ms = 300;

                // 1. Try to resolve session path (%APPDATA%/bundle_id/downloads.json)
                if let Ok(session_path) = get_session_file_path(&app_handle) {
                    if session_path.exists() {
                        // 2. Try to deserialize datas using DownloadDataFactory
                        if let Ok(datas) =
                            DownloadDataFactory::deserialize_datas(&session_path).await
                        {
                            // 3. Reconstruct DownloadManager with saved session
                            if let Ok(reconstructed_manager) =
                                DownloadManager::reconstruct(client.clone(), poll_every_ms, datas)
                                    .await
                            {
                                return reconstructed_manager;
                            }
                        }
                    }
                }

                // Fallback: If no session file exists or parsing failed, create a fresh manager
                DownloadManager::new(client, poll_every_ms)
            });
            let cfg: crate::config_file::Config = match confy::load("dashload", None) {
                Ok(config) => config,
                Err(err) => {
                    eprintln!(
                        "Warning: Failed to load config file ({:?}). Using default configuration.",
                        err
                    );
                    if let Ok(path) = confy::get_configuration_file_path("dashload", None) {
                        if path.exists() {
                            let backup_path = path.with_added_extension("bak");
                            let res = std::fs::rename(&path, &backup_path);
                            if let Err(e) = res {
                                eprintln!(
                                    "Warning: Failed to backup corrupted config file: {:?}",
                                    e
                                );
                            }
                        }
                    }
                    // Fall back safely without crashing the app
                    crate::config_file::Config::default()
                }
            };
            let (config_tx, config_rx) = tokio::sync::watch::channel(cfg);
            let handle_clone = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                events::start_server(handle_clone).await;
            });
            // Register the reconstructed AppState
            let client = reqwest::Client::new();
            app.manage(AppState {
                client,
                download_manager,
                config: config_tx,
                config_rx,
            });

            // ── System Tray ─────────────────────────────────────────────────────
            let show_item = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let tray_icon = Image::from_bytes(include_bytes!("../icons/32x32.png"))
                .expect("Failed to load tray icon bytes");

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|tray, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        let app_handle = tray.app_handle().clone();
                        let session_path = get_session_file_path(&app_handle).ok();
                        tauri::async_runtime::spawn(async move {
                            if let Some(session_path) = session_path {
                                let state = app_handle.state::<AppState>();
                                let _ = state.download_manager.save_session(&session_path).await;
                            }
                            app_handle.exit(0);
                        });
                    }
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Auto-save session before hiding to tray
                let app_handle = window.app_handle().clone();
                let session_path = get_session_file_path(&app_handle).ok();
                tauri::async_runtime::spawn(async move {
                    if let Some(session_path) = session_path {
                        let state = app_handle.state::<AppState>();
                        let _ = state.download_manager.save_session(&session_path).await;
                    }
                });

                // Prevent the window from closing; hide it to the tray instead
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(commands::get_handlers())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
