use crate::external_download_request::ExternalDownloadRequest;
use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

/// Errors related to the HTTP listener server startup.
#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Failed to start the server!")]
    ServerStartError,
}

/// Shared state injected into Axum handlers, providing a thread-safe queue
/// for incoming external download requests.
#[derive(Clone)]
pub struct ServerState {
    /// MPSC sender acting as a bounded queue for `ExternalDownloadRequest` messages.
    pub tx: mpsc::Sender<ExternalDownloadRequest>,
}

/// HTTP server that listens for external download requests (e.g., from browser extensions)
/// and forwards them to an internal channel for processing.
pub struct Server {
    port: u16,
    state: ServerState,
}

impl Server {
    /// Creates a new `Server` instance and returns a receiver for queued requests.
    ///
    /// # Arguments
    /// * `port` - The TCP port to bind to (e.g., 8080).
    ///
    /// Returns a tuple of `(Server, mpsc::Receiver<ExternalDownloadRequest>)`.
    pub fn new(port: u16) -> (Self, mpsc::Receiver<ExternalDownloadRequest>) {
        // Buffer size of 100 pending extension requests
        let (tx, receiver) = mpsc::channel(100);
        let server = Server {
            port,
            state: ServerState { tx },
        };
        (server, receiver)
    }

    /// Starts the Axum HTTP server, listening on `127.0.0.1:port`.
    /// Exposes a `/download` POST endpoint that accepts JSON `ExternalDownloadRequest` payloads.
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

/// Axum handler for the `/download` endpoint.
/// Receives JSON `ExternalDownloadRequest`, enqueues it via the channel, and responds.
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
