use percent_encoding::percent_decode_str;
use reqwest::header::HeaderMap;
use serde::Serialize;
use thiserror::Error;
/// Errors that can occur when extracting a filename from a remote URL.
#[derive(Error, Debug, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum FilepathGettingError {
    #[error("Unreachable Server")]
    UnreachableServer,
    #[error("Request failed")]
    RequestError(String),
    #[error("No content disposition")]
    ContentDispositionLess,
    #[error("No filename")]
    Filenameless,
    #[error("Given url is invalid. Url: {0}")]
    InvalidUrl(String),
}

impl From<reqwest::Error> for FilepathGettingError {
    fn from(e: reqwest::Error) -> Self {
        FilepathGettingError::RequestError(e.to_string())
    }
}


    /// Extracts the file size from a `Content-Length` header map.
    ///
    /// Returns `Some(n)` if the header is present and valid, `None` otherwise.
    pub(crate) fn filesize_from_headers(headers: &HeaderMap) -> Option<u64> {
        headers
            .get("Content-Length")
            .and_then(|len| len.to_str().ok())
            .and_then(|len_str| len_str.parse::<u64>().ok())
    }

pub fn sanitize_filename(raw: &str) -> String {
    // 1. URL-decode the string first (converts %20 -> space, %28 -> (, etc.)
    let decoded = percent_decode_str(raw).decode_utf8_lossy();

    // 2. Explicitly split by BOTH Unix (/) and Windows (\) path separators
    let base_name = decoded
        .trim()
        .split(|c| c == '/' || c == '\\')
        .next_back()
        .unwrap_or("");

    // 3. Replace illegal filesystem characters (including NUL) with '_',
    //    then drop remaining control characters (e.g. \n, \r, \t).
    //    Mapping must happen BEFORE filtering so that '\0' becomes '_' instead
    //    of being silently deleted by the control-character filter.
    let cleaned: String = base_name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\0' => '_',
            c => c,
        })
        .filter(|&c| !c.is_control())
        .collect();

    // 4. Strip leading/trailing dots & spaces (Windows restriction)
    let trimmed = cleaned.trim_matches(|c| c == ' ' || c == '.');

    // 5. Check for Windows reserved device names (case-insensitive)
    let upper = trimmed.to_uppercase();
    let stem = upper.split('.').next().unwrap_or("");
    let reserved_windows = matches!(
        stem,
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );

    if trimmed.is_empty() || trimmed == "." || trimmed == ".." || reserved_windows {
        "download".to_string()
    } else {
        trimmed.to_string()
    }
}
/// Attempts to determine a suitable filename from the remote URL.
/// Tries `HEAD` first, falls back to `GET`, and parses `Content-Disposition` header.
/// Falls back to the last path segment of the URL if no header is present.
pub async fn get_filename_from_url_unsanitized(
    client: reqwest::Client,
    url: &str,
) -> Result<String, FilepathGettingError> {
    let mut res = client.head(url).send().await?;
    if !res.status().is_success() {
        res = client.get(url).send().await?;
        if !res.status().is_success() {
            return Err(FilepathGettingError::UnreachableServer);
        }
    }
    let content_disposition_maybe = res.headers().get("Content-Disposition");
    if let Some(content_disposition_content) = content_disposition_maybe
        && let Ok(content_disposition_str) = content_disposition_content.to_str()
    {
        let parsed = content_disposition::parse_content_disposition(content_disposition_str);
        if let Some(filename) = parsed.filename_full() {
            return Ok(filename);
        }
    }
    let trimmed_url = url.trim();
    if trimmed_url.ends_with("/") {
        return Err(FilepathGettingError::Filenameless);
    }
    let url_parsed =
        url::Url::parse(url).map_err(|_| FilepathGettingError::InvalidUrl(url.to_owned()))?;
    let url_segments_maybe = url_parsed.path_segments();
    if let Some(mut url_segments) = url_segments_maybe {
        let result_or_err = url_segments
            .next_back()
            .map(|seg| seg.to_owned())
            .ok_or(FilepathGettingError::Filenameless);
        return result_or_err;
    }
    Err(FilepathGettingError::Filenameless)
}

pub async fn get_filename_from_url(
    client: reqwest::Client,
    url: &str,
) -> Result<String, FilepathGettingError> {
    let unsanitized = get_filename_from_url_unsanitized(client, url).await?;
    Ok(sanitize_filename(&unsanitized))
}

/// Fetches the `Content-Length` header via `HEAD` request to determine file size.
/// Returns `None` if the request fails or the header is missing/malformed.
pub async fn get_filesize(
    client: reqwest::Client,
    url: &str,
    header_map: HeaderMap,
) -> Option<u64> {
    let req = client.head(url).headers(header_map);
    req.send()
        .await
        .ok()?
        .headers()
        .get("Content-Length")
        .and_then(|len| len.to_str().ok())
        .and_then(|len_str| len_str.parse::<u64>().ok())
}
#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================
    // i. Normal Filenames
    // ==========================================


    #[test]
    fn filesize_from_headers_valid() {
        use reqwest::header::{HeaderMap, HeaderValue};
        let mut headers = HeaderMap::new();
        headers.insert("content-length", HeaderValue::from_static("1234"));
        assert_eq!(crate::info_getters::filesize_from_headers(&headers), Some(1234));
    }

    #[test]
    fn filesize_from_headers_missing() {
        use reqwest::header::HeaderMap;
        let headers = HeaderMap::new();
        assert_eq!(crate::info_getters::filesize_from_headers(&headers), None);
    }
    #[test]
    fn test_normal_filename() {
        assert_eq!(sanitize_filename("document.pdf"), "document.pdf");
        assert_eq!(
            sanitize_filename("my_cool_picture.png"),
            "my_cool_picture.png"
        );
        assert_eq!(
            sanitize_filename("report_2026-final.docx"),
            "report_2026-final.docx"
        );
    }

    #[test]
    fn test_url_encoded_filename() {
        // Solves the exact user problem: URL-encoded strings become clean human text
        let input =
            "Amazing%20Spider-Man%20033%20%282026%29%20%28Digital%29%20%28Shan-Empire%29.cbz";
        let expected = "Amazing Spider-Man 033 (2026) (Digital) (Shan-Empire).cbz";
        assert_eq!(sanitize_filename(input), expected);
    }

    // ==========================================
    // ii. Hostile & Path Traversal Inputs
    // ==========================================
    #[test]
    fn test_path_traversal_unix() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("/var/www/uploads/shell.php"), "shell.php");
        assert_eq!(sanitize_filename("....//....//etc/shadow"), "shadow"); // basename extraction after stripping dot-only segments
    }

    #[test]
    fn test_path_traversal_windows() {
        assert_eq!(
            sanitize_filename("..\\..\\windows\\system32\\cmd.exe"),
            "cmd.exe"
        );
        assert_eq!(
            sanitize_filename("C:\\Users\\Admin\\secret.txt"),
            "secret.txt"
        );
        assert_eq!(
            sanitize_filename("folder/subfolder\\malicious.exe"),
            "malicious.exe"
        );
    }

    // ==========================================
    // iii. Other Edge Cases & Security Vectors
    // ==========================================
    #[test]
    fn test_windows_reserved_names() {
        // Windows crashes or throws exceptions when handling these as filenames
        assert_eq!(sanitize_filename("CON"), "download");
        assert_eq!(sanitize_filename("con.txt"), "download");
        assert_eq!(sanitize_filename("PRN"), "download");
        assert_eq!(sanitize_filename("AUX.log"), "download");
        assert_eq!(sanitize_filename("COM1"), "download");
        assert_eq!(sanitize_filename("LPT9.tar.gz"), "download");
    }

    #[test]
    fn test_illegal_filesystem_characters() {
        let input = "file<name>:with\"illegal*chars?|here*.txt";
        let expected = "file_name__with_illegal_chars__here_.txt";
        assert_eq!(sanitize_filename(input), expected);
    }

    #[test]
    fn test_leading_trailing_dots_and_spaces() {
        // Windows drops trailing spaces/dots, which can cause security extensions bypasses
        assert_eq!(sanitize_filename("   file.txt   "), "file.txt");
        assert_eq!(sanitize_filename("...file.txt..."), "file.txt");
        assert_eq!(sanitize_filename(".hidden_file."), "hidden_file");
    }

    #[test]
    fn test_empty_and_dot_fallbacks() {
        assert_eq!(sanitize_filename(""), "download");
        assert_eq!(sanitize_filename("   "), "download");
        assert_eq!(sanitize_filename("."), "download");
        assert_eq!(sanitize_filename(".."), "download");
    }

    #[test]
    fn test_control_characters() {
        assert_eq!(sanitize_filename("file\u{0000}name.txt"), "file_name.txt");
        assert_eq!(sanitize_filename("file\nname\r.txt"), "filename.txt");
    }

    #[test]
    fn test_unicode_and_international_characters() {
        // Ensure non-ASCII UTF-8 characters are safely preserved
        assert_eq!(sanitize_filename("résumé_2026.pdf"), "résumé_2026.pdf");
        assert_eq!(sanitize_filename("测试文件.txt"), "测试文件.txt");
        assert_eq!(sanitize_filename("файл.txt"), "файл.txt");
    }
}
