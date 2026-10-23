use crate::api_error::ApiError;
use downloader::download_data::DownloadData;
use downloader::download_manager::{DownloadManager, ManagerError};
use downloader::download_status::DownloadStatus;
use downloader::get_download_info::{get_download_info, DownloadInfo, DownloadInfoGetError};
use downloader::request_headers::RequestHeaders;
use downloader::serialization_error::DataSerializationError;
use downloader::verifier::VerifierMaybe;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::ipc::Invoke;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;

/// Resolves cross-platform app data path: %APPDATA%/bundle_id/downloads.json
pub fn get_session_file_path(app: &AppHandle) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let app_data_dir = app.path().app_data_dir()?;
    if !app_data_dir.exists() {
        std::fs::create_dir_all(&app_data_dir)?;
    }
    Ok(app_data_dir.join("downloads.json"))
}

// Ensure these fields are pub if created in lib.rs
pub struct AppState {
    pub client: reqwest::Client,
    pub download_manager: DownloadManager,
    pub config: crate::config_file::Config,
}

#[tauri::command]
async fn add_download(
    state: State<'_, AppState>,
    url: &str,
    filepath: &str,
    number_of_segments: u64,
    verifier: VerifierMaybe,
    request_headers: RequestHeaders,
) -> Result<String, ApiError<ManagerError>> {
    Ok(state
        .download_manager
        .add_download(
            url.to_string(),
            filepath.into(),
            number_of_segments,
            verifier,
            request_headers,
        )
        .await?)
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
#[tauri::command]
async fn get_config_file(state: State<'_, AppState>) -> Result<crate::config_file::Config, ()> {
    Ok(state.config.clone())
}

#[tauri::command]
fn get_status(state: State<'_, AppState>) -> HashMap<String, DownloadStatus> {
    state.download_manager.get_all_items()
}

#[tauri::command]
fn get_item_status(
    state: State<'_, AppState>,
    id: &str,
) -> Result<DownloadStatus, ApiError<ManagerError>> {
    Ok(state.download_manager.get_item_status(id)?)
}

#[tauri::command]
async fn get_download_metadata(
    state: State<'_, AppState>,
    url: &str,
) -> Result<DownloadInfo, ApiError<DownloadInfoGetError>> {
    Ok(get_download_info(state.client.clone(), url).await?)
}

#[tauri::command]
fn pause(state: State<'_, AppState>, id: &str) -> Result<(), ApiError<ManagerError>> {
    state.download_manager.pause(id)?;
    Ok(())
}

#[tauri::command]
async fn resume(state: State<'_, AppState>, id: &str) -> Result<(), ApiError<ManagerError>> {
    state.download_manager.resume(id).await?;
    Ok(())
}

#[tauri::command]
fn get_data(state: State<'_, AppState>) -> HashMap<String, DownloadData> {
    state.download_manager.get_full_snapshot()
}

#[tauri::command]
async fn save_session(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), ApiError<DataSerializationError>> {
    let path = get_session_file_path(&app)
        .map_err(|e| ApiError::new(DataSerializationError::IoError(e.to_string())))?;
    state.download_manager.save_session(&path).await?;
    Ok(())
}

pub fn get_handlers() -> impl Fn(Invoke<tauri::Wry>) -> bool {
    tauri::generate_handler![
        add_download,
        open_file,
        get_status,
        get_item_status,
        get_download_metadata,
        pause,
        resume,
        get_data,
        save_session,
        get_config_file
    ]
}
