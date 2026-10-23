use crate::external_download_request::ExternalDownloadRequest;
use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Failed to start the server!")]
    ServerStartError,
}

// Shared state that Axum handlers can safely clone and access across threads
#[derive(Clone)]
pub struct ServerState {
    // mpsc sender acts as your queue (replaces the watch channel)
    pub tx: mpsc::Sender<ExternalDownloadRequest>,
}

pub struct Server {
    port: u16,
    state: ServerState,
}

impl Server {
    pub fn new(port: u16) -> (Self, mpsc::Receiver<ExternalDownloadRequest>) {
        // Buffer size of 100 pending extension requests
        let (tx, receiver) = mpsc::channel(100);
        let server = Server {
            port,
            state: ServerState { tx },
        };
        (server, receiver)
    }

    pub async fn start(self) -> Result<(), ServerError> {
        let app = Router::new()
            .route("/download", post(handle_download_request))
            .with_state(self.state); // Inject state into Axum router

        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|_| ServerError::ServerStartError)?;

        axum::serve(listener, app)
            .await
            .map_err(|_| ServerError::ServerStartError)?;

        Ok(())
    }
}

// Standalone or associated handler function leveraging Axum's State extractor
async fn handle_download_request(
    State(state): State<ServerState>,
    Json(request): Json<ExternalDownloadRequest>,
) -> Result<Json<String>, StatusCode> {
    // Send the request into the mpsc queue instead of overwriting it
    state.tx.send(request).await.map_err(|_| {
        // Channel receiver dropped or failed
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json("Download request queued successfully".to_string()))
}
