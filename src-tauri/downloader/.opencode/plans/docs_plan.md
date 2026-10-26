# Documentation Plan: `dashload-downloader`

## 1. Primary User-Facing Workflow
The `dashload-downloader` library is designed for managing single-file and multi-segmented (parallel chunked) HTTP downloads with support for pause, resume, checksum verification, and state persistence.

The standard lifecycle workflow:
1. **Initialize Client & Manager**: Create a `reqwest::Client` and instantiate the `DownloadManager` with a desired polling interval (e.g., `400ms`).
2. **Configure Request & Verification**: Set up any required `RequestHeaders` (User-Agent, Referrer, Cookies) and an optional `VerifierMaybe` (SHA256, SHA512, MD5) for post-download integrity checking.
3. **Add Download**: Call `download_manager.add_download(...)` passing the URL, destination `PathBuf`, number of segments, verifier, and headers. The manager automatically checks for file size to determine if multi-segmenting is supported, spawns a background Tokio task, and returns a unique string ID.
4. **Monitor / Poll Status**: Periodically call `download_manager.get_item_status(&id)` to inspect progress (`downloaded`, `total`, and `DownloadState`). Match against states like `DownloadState::Running`, `DownloadState::Completed { size }`, `DownloadState::Paused`, or `DownloadState::Failed(err)`.
5. **Control Lifecycles**: Pause (`download_manager.pause(&id)`) or resume (`download_manager.resume(&id).await`) downloads dynamically.

---

## 2. Core Public Types
- **`DownloadManager`**: The core engine managing active download items via a thread-safe `DashMap`. Handles creation, persistence, pause/resume, and status tracking.
- **`DownloadStatus`**: Struct containing `total` (`Option<u64>`), `downloaded` (`u64`), and `state` (`DownloadState`).
- **`DownloadState`**: Enum representing the current state of a download item:
  - `Stopped`
  - `Running { download_speed: f64 }`
  - `Completed { size: u64 }`
  - `Failed(DownloadRunError)`
  - `Paused`
  - `Verifying`
- **`VerifierMaybe`**: Wrapper around an optional hash verifier (`Verifier`) supporting algorithms (`HashAlgo::Sha256`, `HashAlgo::Sha512`, `HashAlgo::Md5`). Constructed via `VerifierMaybe::new(...)` or `VerifierMaybe::none()`.
- **`RequestHeaders`**: Struct containing optional HTTP headers (`user_agent`, `referrer`, `cookies`) mapped to `reqwest::header::HeaderMap`.

---

## 3. Code Examples (Derived from Integration Tests)

### Example 1: Segmented Download with Pause & Resume Lifecycle
```rust
use downloader::download_manager::DownloadManager;
use downloader::download_status::DownloadState;
use downloader::request_headers::RequestHeaders;
use downloader::verifier::VerifierMaybe;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 200);
    let path = PathBuf::from("./downloaded_file.iso");

    let request_headers = RequestHeaders {
        user_agent: Some("Mozilla/5.0 (Windows NT 10.0; rv:128.0) Gecko/20100101 Firefox/128.0".to_string()),
        referrer: Some("https://example.com/".to_string()),
        cookies: None,
    };

    // 1. Add download with 4 segments and no verifier
    let id = manager
        .add_download(
            "https://example.com/file.iso".to_string(),
            path.clone(),
            4,
            VerifierMaybe::none(),
            request_headers,
        )
        .await?;

    // Let it run for a moment
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // 2. Pause the download
    manager.pause(&id)?;
    println!("Download paused.");

    // 3. Resume the download
    manager.resume(&id).await?;
    println!("Download resumed.");

    // Cleanup test file
    let _ = tokio::fs::remove_file(&path).await;
    Ok(())
}
```

### Example 2: Download with Automated SHA256 Verification
```rust
use downloader::download_manager::DownloadManager;
use downloader::download_status::DownloadState;
use downloader::request_headers::RequestHeaders;
use downloader::verifier::{HashAlgo, VerifierMaybe};
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client.clone(), 200);
    let path = PathBuf::from("./alpine-minirootfs.tar.gz");

    let download_url = "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/alpine-minirootfs-3.24.1-x86_64.tar.gz";
    
    // Example expected hash string
    let expected_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let verifier = VerifierMaybe::new(HashAlgo::Sha256, expected_hash.to_string());

    let request_headers = RequestHeaders {
        user_agent: Some("Mozilla/5.0 (Windows NT 10.0; rv:128.0) Gecko/20100101 Firefox/128.0".to_string()),
        referrer: Some("https://alpinelinux.org/".to_string()),
        cookies: None,
    };

    let id = manager
        .add_download(
            download_url.to_string(),
            path.clone(),
            4,
            verifier,
            request_headers,
        )
        .await?;

    loop {
        let status = manager.get_item_status(&id)?;
        match status.state {
            DownloadState::Completed { size } => {
                println!("Download finished and successfully verified! Size: {} bytes", size);
                break;
            }
            DownloadState::Failed(err) => {
                eprintln!("Download or verification failed: {:?}", err);
                break;
            }
            _ => {}
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    let _ = tokio::fs::remove_file(&path).await;
    Ok(())
}
```
