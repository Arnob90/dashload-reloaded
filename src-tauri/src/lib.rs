use tauri::Manager;
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
            let handle_clone = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                events::start_server(handle_clone).await;
            });
            // Register the reconstructed AppState
            let client = reqwest::Client::new();
            app.manage(AppState {
                client,
                download_manager,
                config: cfg,
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // Auto-save session on close
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let app_handle = window.app_handle().clone();
                tauri::async_runtime::block_on(async move {
                    if let Ok(session_path) = get_session_file_path(&app_handle) {
                        let state = app_handle.state::<AppState>();
                        let _ = state.download_manager.save_session(&session_path).await;
                    }
                });
            }
        })
        .invoke_handler(commands::get_handlers())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
