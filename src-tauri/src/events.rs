use downloader::listener::Server;
use tauri::{AppHandle, Emitter};
pub async fn start_server(app: AppHandle) {
    let (server, mut receiver) = Server::new(3000);
    tauri::async_runtime::spawn(async move {
        if let Err(e) = server.start().await {
            eprintln!("Failed to start server: {}", e);
        }
    });
    while let Some(request) = receiver.recv().await {
        let _ = app.emit("external-download-request", request);
    }
}
