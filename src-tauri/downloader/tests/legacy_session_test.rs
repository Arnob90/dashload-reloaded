use dashload_downloader::download_run_error::{DownloadRunError, FailureDetails};
use dashload_downloader::download_status::DownloadState;

#[test]
fn legacy_failed_network_error_loads() {
    // Legacy format pre-b045fde: Failed(DownloadRunError) with tag/content enums.
    let legacy = r#"{"kind":"Failed","payload":{"kind":"NetworkError","payload":"connection reset"}}"#;
    let state: Result<DownloadState, _> = serde_json::from_str(legacy);
    match state {
        Ok(DownloadState::Failed(f)) => {
            assert_eq!(f.error, DownloadRunError::NetworkError { message: "connection reset".into(), status_code: None });
        }
        other => panic!("legacy NetworkError failed to load: {:?}", other),
    }
}

#[test]
fn legacy_failed_server_error_loads() {
    let legacy = r#"{"kind":"Failed","payload":{"kind":"ServerError","payload":"Server returned HTTP 404"}}"#;
    match serde_json::from_str::<DownloadState>(legacy) {
        Ok(DownloadState::Failed(f)) => println!("{}", f.error),
        other => panic!("legacy ServerError failed to load: {:?}", other),
    }
}

#[test]
fn legacy_failed_disk_error_loads() {
    let legacy = r#"{"kind":"Failed","payload":{"kind":"DiskError","payload":"disk full"}}"#;
    match serde_json::from_str::<DownloadState>(legacy) {
        Ok(DownloadState::Failed(f)) => println!("{}", f.error),
        other => panic!("legacy DiskError failed to load: {:?}", other),
    }
}

#[test]
fn legacy_failed_verify_error_loads() {
    let legacy = r#"{"kind":"Failed","payload":{"kind":"VerifyError"}}"#;
    match serde_json::from_str::<DownloadState>(legacy) {
        Ok(DownloadState::Failed(_)) => {}
        other => panic!("legacy VerifyError failed to load: {:?}", other),
    }
}

#[test]
fn legacy_failed_timeout_error_loads() {
    let legacy = r#"{"kind":"Failed","payload":{"kind":"TimeoutError"}}"#;
    match serde_json::from_str::<DownloadState>(legacy) {
        Ok(DownloadState::Failed(_)) => {}
        other => panic!("legacy TimeoutError failed to load: {:?}", other),
    }
}

#[test]
fn new_format_roundtrips() {
    let details = FailureDetails::new(DownloadRunError::ServerError { message: "m".into(), status_code: Some(500) }).with_attempts(3);
    let s = serde_json::to_string(&DownloadState::Failed(details.clone())).unwrap();
    let back: DownloadState = serde_json::from_str(&s).unwrap();
    assert_eq!(DownloadState::Failed(details), back);
}

#[test]
fn intermediate_format_failed_loads() {
    // Format from commit 8f74573: Failed(bare DownloadRunError, struct payloads)
    let intermediate = r#"{"kind":"Failed","payload":{"kind":"NetworkError","payload":{"message":"boom","status_code":null}}}"#;
    match serde_json::from_str::<DownloadState>(intermediate) {
        Ok(DownloadState::Failed(f)) => assert_eq!(f.error, DownloadRunError::NetworkError { message: "boom".into(), status_code: None }),
        other => panic!("intermediate format failed to load: {:?}", other),
    }
}
