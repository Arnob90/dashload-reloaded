use downloader::download_data::DownloadData;
use downloader::download_data_factory::DownloadDataFactory;
use downloader::download_manager::DownloadManager;
use downloader::download_status::DownloadStatus;
use downloader::get_download_info::{get_download_info, DownloadInfo};
use downloader::get_error_message::get_error_message;
use downloader::info_getters::get_filename_from_url;
use downloader::verifier::VerifierMaybe;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Display;
use std::path::Path;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;

fn get_error_message_wrap<T: Serialize + Display>(error: T) -> Value {
    get_error_message(error).unwrap_or(Value::Null)
}
/// Resolves cross-platform app data path: %APPDATA%/bundle_id/downloads.json
fn get_session_file_path(app: &AppHandle) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let app_data_dir = app.path().app_data_dir()?;
    if !app_data_dir.exists() {
        std::fs::create_dir_all(&app_data_dir)?;
    }
    Ok(app_data_dir.join("downloads.json"))
}

struct AppState {
    client: reqwest::Client,
    download_manager: DownloadManager,
}

/// Unified command for both 1-segment and multi-segment downloads.
/// Returns the generated UUID String to the frontend.
#[tauri::command]
async fn add_download(
    state: State<'_, AppState>,
    url: &str,
    filepath: &str,
    number_of_segments: u64,
    verifier: VerifierMaybe,
) -> Result<String, Value> {
    let manager = &state.download_manager;
    let id = manager
        .add_download(
            url.to_string(),
            filepath.into(),
            number_of_segments,
            verifier,
        )
        .await
        .map_err(get_error_message_wrap)?;
    Ok(id)
}

#[tauri::command]
async fn open_file(app: AppHandle) -> Option<String> {
    let (sender, receiver) = oneshot::channel::<Option<String>>();
    app.dialog().file().pick_folder(move |folder_chosen| {
        let chosen_folder_str = folder_chosen.map(|f| f.to_string());
        let _ = sender.send(chosen_folder_str);
    });
    match receiver.await {
        Ok(file_path_option) => Some(file_path_option?),
        Err(_) => None,
    }
}

/// Synchronous, lock-free status readout for all downloads.
#[tauri::command]
fn get_status(state: State<'_, AppState>) -> HashMap<String, DownloadStatus> {
    state.download_manager.get_all_items()
}

#[tauri::command]
fn get_filename_from_path(filepath: &Path) -> Option<String> {
    filepath
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

#[tauri::command]
async fn get_download_metadata(
    state: State<'_, AppState>,
    url: &str,
) -> Result<DownloadInfo, Value> {
    get_download_info(state.client.clone(), url)
        .await
        .map_err(get_error_message_wrap)
}

#[tauri::command]
async fn get_filename(state: State<'_, AppState>, url: &str) -> Result<String, Value> {
    let client = state.client.clone();
    get_filename_from_url(client, url)
        .await
        .map_err(get_error_message_wrap)
}

#[tauri::command]
fn join_paths(first_segment: String, second_segment: String) -> String {
    Path::new(&first_segment)
        .join(Path::new(&second_segment))
        .to_string_lossy()
        .to_string()
}

/// Synchronous pause operation
#[tauri::command]
fn pause(state: State<'_, AppState>, id: &str) -> Result<(), Value> {
    state
        .download_manager
        .pause(id)
        .map_err(get_error_message_wrap)?;
    Ok(())
}

/// Async resume operation
#[tauri::command]
async fn resume(state: State<'_, AppState>, id: &str) -> Result<(), Value> {
    state
        .download_manager
        .resume(id)
        .await
        .map_err(get_error_message_wrap)?;
    Ok(())
}

#[tauri::command]
fn get_data(state: State<'_, AppState>) -> HashMap<String, DownloadData> {
    state.download_manager.get_full_snapshot()
}

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

            // Register the reconstructed AppState
            let client = reqwest::Client::new();
            app.manage(AppState {
                client,
                download_manager,
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
        .invoke_handler(tauri::generate_handler![
            add_download,
            get_status,
            pause,
            resume,
            get_data,
            open_file,
            get_download_metadata,
            join_paths,
            get_filename,
            get_filename_from_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
