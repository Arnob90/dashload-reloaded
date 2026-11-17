use serde::Serialize;
use thiserror::Error;

/// Errors that can occur when retrieving the content length from a server.
#[derive(Error, Debug, Serialize, Clone)]
#[serde(tag = "kind", content = "payload")]
pub enum LengthGettingErrors {
    #[error("Request failed!")]
    RequestError(String),
    #[error("The server does not have a concept of filesize")]
    LengthlessServerError,
    #[error("The server violates basic protocols")]
    IllBehavedServer,
}

impl From<reqwest::Error> for LengthGettingErrors {
    fn from(value: reqwest::Error) -> Self {
        Self::RequestError(value.to_string())
    }
}

/// Extracts the content length from a `Content-Length` header string.
///
/// Returns `Ok(n)` for a valid decimal u64 string. Returns
/// `IllBehavedServer` for empty, non-numeric, or negative values
/// (u64 parsing rejects `-1`).
pub(crate) fn length_from_header(value: &str) -> Result<u64, LengthGettingErrors> {
    let length_num: u64 = value
        .parse()
        .map_err(|_| LengthGettingErrors::IllBehavedServer)?;
    Ok(length_num)
}

/// Extracts the `Content-Length` from a `reqwest::Response` header.
/// Returns error if header is missing, not valid UTF-8, or not a valid u64.
pub async fn get_length_from_response(
    response: reqwest::Response,
) -> Result<u64, LengthGettingErrors> {
    let length_str = response
        .headers()
        .get("Content-Length")
        .ok_or(LengthGettingErrors::LengthlessServerError)?
        .to_str()
        .map_err(|_| LengthGettingErrors::IllBehavedServer)?;

    length_from_header(length_str)
}

/// Retrieves the file size (content length) for a given URL.
/// First attempts a `HEAD` request, falls back to `GET` if `HEAD` fails
/// or if the `Content-Length` header is absent.
pub async fn get_length(client: reqwest::Client, url: &str) -> Result<u64, LengthGettingErrors> {
    // First try HEAD
    let response = client.head(url).send().await;
    if response.is_err() {
        let try_get = client.get(url).send().await?;
        return get_length_from_response(try_get).await;
    }
    let length = get_length_from_response(response.unwrap()).await;
    if let Err(LengthGettingErrors::LengthlessServerError) = length {
        let try_get = client.get(url).send().await?;
        return get_length_from_response(try_get).await;
    }
    length
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_from_header_valid() {
        assert_eq!(length_from_header("123").unwrap(), 123);
        assert_eq!(length_from_header("0").unwrap(), 0);
        assert_eq!(length_from_header("18446744073709551615").unwrap(), 18_446_744_073_709_551_615);
    }

    #[test]
    fn length_from_header_errors() {
        assert!(matches!(length_from_header(""), Err(LengthGettingErrors::IllBehavedServer)));
        assert!(matches!(length_from_header("abc"), Err(LengthGettingErrors::IllBehavedServer)));
        assert!(matches!(length_from_header("-5"), Err(LengthGettingErrors::IllBehavedServer)));
        assert!(matches!(length_from_header("1.5"), Err(LengthGettingErrors::IllBehavedServer)));
    }
}
