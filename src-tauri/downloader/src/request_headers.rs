use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};

/// Optional HTTP headers to include with download requests.
/// All fields are optional; missing fields are omitted from the request.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RequestHeaders {
    /// `Cookie` header value.
    pub cookies: Option<String>,
    /// `Referer` header value (note: HTTP header is spelled "Referer").
    pub referrer: Option<String>,
    /// `User-Agent` header value.
    pub user_agent: Option<String>,
}

impl RequestHeaders {
    /// Converts the struct into a `reqwest::header::HeaderMap` suitable for adding to a request.
    /// Returns an error if any header value contains invalid characters.
    pub fn to_header_map(&self) -> Result<HeaderMap, reqwest::header::InvalidHeaderValue> {
        let mut map = HeaderMap::new();

        if let Some(cookies) = &self.cookies {
            map.insert(
                HeaderName::from_static("cookie"),
                HeaderValue::from_str(cookies)?,
            );
        }

        if let Some(referrer) = &self.referrer {
            map.insert(
                HeaderName::from_static("referer"),
                HeaderValue::from_str(referrer)?,
            );
        }

        if let Some(user_agent) = &self.user_agent {
            map.insert(
                HeaderName::from_static("user-agent"),
                HeaderValue::from_str(user_agent)?,
            );
        }

        Ok(map)
    }
    pub fn from_header_map(map: &HeaderMap) -> Self {
        let cookies = map
            .get("cookie")
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string());

        let referrer = map
            .get("referer")
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string());

        let user_agent = map
            .get("user-agent")
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string());

        RequestHeaders {
            cookies,
            referrer,
            user_agent,
        }
    }
}
