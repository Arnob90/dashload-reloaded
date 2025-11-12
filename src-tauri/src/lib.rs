use downloader::idownloaditem::IDownloadData;
use downloader::segment::DownloadStatusCheckError;
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use downloader::download_manager::DownloadManager;
use downloader::download_manager::SmartDownloadItemError;
use downloader::filename_getter;
use downloader::get_download_info::get_download_info;
use downloader::get_download_info::DownloadInfo;
use downloader::get_download_info::DownloadInfoGetError;
use downloader::normal_download_item::NormalDownloadItemCreationError;
use downloader::segmented_download_item::SegmentedDownloadItemCreationError;
use std::collections::HashMap;
use std::path::Path;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;
use tokio::sync::Mutex;

struct AppState {
    client: reqwest::Client,
    download_manager: Mutex<DownloadManager>,
}

impl AppState {
    pub fn new() -> AppState {
        let client = reqwest::Client::new();
        Self {
            client: client.clone(),
            download_manager: Mutex::new(DownloadManager::new(client.clone(), 300)),
        }
    }
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn start_download_normal(
    state: State<'_, AppState>,
    given_url: &str,
    filepath: &str,
) -> Result<(), NormalDownloadItemCreationError> {
    let mut manager = state.download_manager.lock().await;
    manager
        .add_download_normal(given_url.to_string(), filepath.to_string())
        .await?;
    Ok(())
}

#[tauri::command]
async fn start_download_segmented(
    state: State<'_, AppState>,
    given_url: &str,
    filepath: &str,
    number_of_segments: u64,
) -> Result<(), SegmentedDownloadItemCreationError> {
    let mut manager = state.download_manager.lock().await;
    manager
        .add_download_segmented(
            given_url.to_string(),
            filepath.to_string(),
            number_of_segments,
        )
        .await?;
    Ok(())
}

#[tauri::command]
async fn start_download_smart(
    state: State<'_, AppState>,
    given_url: &str,
    filepath: &str,
) -> Result<(), SmartDownloadItemError> {
    let mut manager = state.download_manager.lock().await;
    manager
        .add_download_smart(given_url.to_string(), filepath.to_string())
        .await?;
    Ok(())
}

#[tauri::command]
async fn open_file(app: AppHandle) -> Result<Option<String>, String> {
    let (sender, reciever) = oneshot::channel::<Option<String>>();
    app.dialog().file().pick_folder(|folder_choosen_maybe| {
        let choosen_folder_str = folder_choosen_maybe.map(|file_choosen| file_choosen.to_string());
        let _ = sender.send(choosen_folder_str);
    });
    match reciever.await {
        Ok(file_path_option) => Ok(file_path_option),
        Err(_) => Err("Failed to get file!".to_string()),
    }
}

#[tauri::command]
async fn get_status(
    state: State<'_, AppState>,
) -> Result<HashMap<String, IDownloadData>, DownloadStatusCheckError> {
    let manager = state.download_manager.lock().await;
    let status = manager.get_all_items().await?;
    Ok(status)
}
#[tauri::command]
async fn get_download_metadata(
    state: State<'_, AppState>,
    url: &str,
) -> Result<DownloadInfo, DownloadInfoGetError> {
    get_download_info(state.client.clone(), url).await
}
#[tauri::command]
async fn get_filename(
    state: State<'_, AppState>,
    url: &str,
) -> Result<String, filename_getter::FilepathGettingError> {
    let client = state.client.clone();
    filename_getter::get_filename(client, url).await
}

#[tauri::command]
async fn join_paths(first_segment: String, second_segment: String) -> String {
    let first_segment_path = Path::new(&first_segment);
    let second_segment_path = Path::new(&second_segment);
    let joined_path = first_segment_path.join(second_segment_path);
    joined_path.to_string_lossy().to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(AppState::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            start_download_normal,
            open_file,
            get_status,
            start_download_segmented,
            start_download_smart,
            get_download_metadata,
            get_filename,
            join_paths
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
