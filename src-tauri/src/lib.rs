use downloader::download_manager::SegmentedDownloadAddErrors;
use downloader::idownloaditem::IDownloadData;
use downloader::segment::DownloadStatusCheckError;
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use downloader::download_manager::DownloadManager;
use std::collections::HashMap;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;
use tokio::sync::Mutex;
struct DownloadManagerContainer {
    download_manager: Mutex<DownloadManager>,
}

impl DownloadManagerContainer {
    pub fn new() -> DownloadManagerContainer {
        //Default 300ms
        Self {
            download_manager: Mutex::new(DownloadManager::new(300)),
        }
    }
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn start_download_normal(
    state: State<'_, DownloadManagerContainer>,
    given_url: &str,
    filepath: &str,
) -> Result<(), ()> {
    let mut manager = state.download_manager.lock().await;
    manager
        .add_download_normal(given_url.to_string(), filepath.to_string())
        .await;
    Ok(())
}

#[tauri::command]
async fn start_download_segmented(
    state: State<'_, DownloadManagerContainer>,
    given_url: &str,
    filepath: &str,
    number_of_segments: u64,
) -> Result<(), SegmentedDownloadAddErrors> {
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
    state: State<'_, DownloadManagerContainer>,
) -> Result<HashMap<String, IDownloadData>, DownloadStatusCheckError> {
    let manager = state.download_manager.lock().await;
    let status = manager.get_all_items().await?;
    Ok(status)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(DownloadManagerContainer::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            start_download_normal,
            open_file,
            get_status,
            start_download_segmented
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
