use serde::{Deserialize, Serialize};

use crate::request_headers::RequestHeaders;

/// Payload received from external sources (e.g., browser extensions) to initiate a download.
#[derive(Deserialize, Debug, Clone, Default, Serialize)]
pub struct ExternalDownloadRequest {
    /// The download URL.
    pub url: String,
    /// Optional suggested filename (may be inferred from headers if absent).
    pub filename: Option<String>,
    /// Optional known file size in bytes.
    pub filesize: Option<u64>,
    /// Custom HTTP headers to use for the download request.
    pub headers: RequestHeaders,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal payload (only `url` + `headers`) deserializes with the
    /// optional fields defaulting to `null`.
    #[test]
    fn minimal_payload_deserializes() {
        let json = r#"{
            "url": "https://example.com/file.bin",
            "headers": {}
        }"#;
        let req: ExternalDownloadRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.url, "https://example.com/file.bin");
        assert_eq!(req.filename, None);
        assert_eq!(req.filesize, None);
        assert_eq!(req.headers.cookies, None);
        assert_eq!(req.headers.referrer, None);
        assert_eq!(req.headers.user_agent, None);
    }

    /// Full payload with all fields round-trips through serde.
    #[test]
    fn full_payload_roundtrip() {
        let req = ExternalDownloadRequest {
            url: "https://example.com/file.bin".to_string(),
            filename: Some("file.bin".to_string()),
            filesize: Some(1_000_000),
            headers: RequestHeaders {
                cookies: Some("session=abc".to_string()),
                referrer: Some("https://example.com/".to_string()),
                user_agent: Some("curl/8.0".to_string()),
            },
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: ExternalDownloadRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.url, back.url);
        assert_eq!(req.filename, back.filename);
        assert_eq!(req.filesize, back.filesize);
        assert_eq!(req.headers, back.headers);
    }
}
