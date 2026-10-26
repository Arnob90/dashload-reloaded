# dashload-downloader

A high-performance, asynchronous HTTP file downloader for Rust with support for **multi-segment (parallel) downloads**, **pause/resume**, **checksum verification**, and **session persistence**.

Built on `tokio` and `reqwest`, designed for CLI tools, desktop apps (Tauri), and backend services.

---

## Features

| Feature | Description |
| --------- | ------------- |
| **Multi-segment downloads** | Splits large files into concurrent chunks for maximum throughput |
| **Pause & Resume** | Stop and continue downloads at any time; state survives process restarts |
| **Checksum verification** | Automatic SHA-256 / SHA-512 / MD5 validation after download completes |
| **Session persistence** | Serialize full download state to JSON; reconstruct on startup |
| **Custom headers** | Per-download User-Agent, Referer, Cookie support |
| **Progress polling** | Configurable update interval; real-time speed & progress via watch channels |
| **Segment pre-flight check** | Validates `Accept-Ranges` support before committing to segmented download |
| **Single & multi-file** | Works for both known-size (segmented) and unknown-size (streaming) downloads |

---

## Installation

```fish
cargo add dashload-downloader
```

---

## Quick Start

```rust
use dashload_downloader::{
    download_manager::DownloadManager,
    request_headers::RequestHeaders,
    verifier::{HashAlgo, VerifierMaybe},
};
use reqwest::Client;
use std::path::PathBuf;
use tokio::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let manager = DownloadManager::new(client, 500); // 500ms poll interval

    // Optional: verify SHA-256 after download
    let verifier = VerifierMaybe::new(
        HashAlgo::Sha256,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
    );

    // Optional: custom headers
    let headers = RequestHeaders {
        user_agent: Some("MyApp/1.0".into()),
        referrer: Some("https://example.com/".into()),
        cookies: None,
    };

    let id = manager
        .add_download(
            "https://example.com/large-file.iso".into(),
            PathBuf::from("./downloads/large-file.iso"),
            4,              // 4 concurrent segments
            verifier,
            headers,
        )
        .await?;

    // Poll for progress
    loop {
        let status = manager.get_item_status(&id)?;
        match status.state {
            dashload_downloader::download_status::DownloadState::Completed { size } => {
                println!("Done! {} bytes", size);
                break;
            }
            dashload_downloader::download_status::DownloadState::Failed(e) => {
                eprintln!("Failed: {}", e);
                break;
            }
            dashload_downloader::download_status::DownloadState::Running { download_speed } => {
                println!("{:.1} MB/s  ({}/{})",
                    download_speed / 1_000_000.0,
                    status.downloaded,
                    status.total.unwrap_or(0)
                );
            }
            _ => {}
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Ok(())
}
```

---

## Core Concepts

### DownloadManager

Central coordinator. Holds a thread-safe map of active downloads (`DashMap`), a shared `reqwest::Client`, and a polling interval.

```rust
let manager = DownloadManager::new(client, 500);
```

### Download Lifecycle

```
add_download()  →  Running  →  (pause)  →  Paused  →  (resume)  →  Running
                                                            ↓
                                                    Completed / Failed
```

### VerifierMaybe

Wraps an optional hash check. Use `VerifierMaybe::none()` to skip verification.

```rust
// With verification
let v = VerifierMaybe::new(HashAlgo::Sha256, "expected_hex_hash".into());

// Without
let v = VerifierMaybe::none();
```

### RequestHeaders

Optional HTTP headers sent with every segment request.

```rust
let h = RequestHeaders {
    user_agent: Some("Mozilla/5.0 ...".into()),
    referrer: Some("https://site.com/".into()),
    cookies: Some("session=xyz".into()),
};
```

### DownloadState

```rust
enum DownloadState {
    Stopped,
    Running { download_speed: f64 },  // bytes/sec
    Completed { size: u64 },
    Failed(DownloadRunError),
    Paused,
    Verifying,
}
```

---

## Advanced Usage

### Pause & Resume

```rust
manager.pause(&id)?;                    // synchronous
// ... later ...
manager.resume(&id).await?;             // async (recreates segments)
```

### Session Persistence

```rust
// Save all downloads to disk
manager.save_session(&PathBuf::from("session.json")).await?;

// Restore on startup
let (manager, _rx) = DownloadManager::reconstruct(client, 500, saved_data).await?;
```

### Get All Statuses

```rust
let all: HashMap<String, DownloadStatus> = manager.get_all_items();
```

### External Request Server (for browser extensions)

```rust
use dashload_downloader::listener::{Server, ServerState};
use tokio::sync::mpsc;

let (server, mut rx) = Server::new(8080);
tokio::spawn(async move { server.start().await });

while let Some(req) = rx.recv().await {
    // req: ExternalDownloadRequest { url, filename, filesize, headers }
    manager.add_download(...).await?;
}
```

---

## Architecture Overview

```
DownloadManager
    │
    ├── DashMap<id, Arc<DownloadItem>>
    │
    └── DownloadItem (per download)
            │
            ├── Vec<InPlaceSegment>  (1 for normal, N for segmented)
            │
            ├── watch::Sender<SegmentCommand>  → pause/stop all segments
            │
            ├── Vec<watch::Receiver<DownloadStatus>>  ← progress from each segment
            │
            └── VerifierMaybe  → spawn_blocking verification on completion
```

Each `InPlaceSegment`:

- Opens the target file at its byte offset (`seek`)
- Issues ranged `GET` requests (`Range: bytes=start-end`)
- Streams chunks directly to disk via `AsyncWriteExt`
- Reports progress via `watch::Sender<DownloadStatus>`
- Listens for `SegmentCommand::Pause/Stop` via `watch::Receiver`

---

## Error Types

| Error | Source |
| ------- | -------- |
| `ManagerError::IdDoesNotExist` | Invalid download ID |
| `ManagerError::DownloadCreationError` | Segment setup failed |
| `ManagerError::OperationFailed` | Resume/pause failed |
| `DownloadRunError::NetworkError` | DNS, TCP, TLS failure |
| `DownloadRunError::ServerError` | 4xx/5xx, no range support |
| `DownloadRunError::DiskError` | Permission, disk full, path |
| `DownloadRunError::VerifyError` | Hash mismatch |
| `SegmentVerificationError` | Pre-flight range check failed |

All errors implement `std::error::Error`, `Serialize`, `Deserialize`.

---

## Running Tests

```bash
# Unit tests (fast, mocked)
cargo test

# Integration tests (real HTTP, marked #[ignore])
cargo test -- --ignored
```

Integration tests download real files (Linux Mint ISO, Alpine minirootfs) and verify SHA-256.

---

## License

MIT OR Apache-2.0
