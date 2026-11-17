use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};

/// Optional HTTP headers to include with download requests.
/// All fields are optional; missing fields are omitted from the request.
#[derive(Clone, Debug, Serialize, Deserialize)]
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

impl Default for RequestHeaders {
    fn default() -> Self {
        RequestHeaders {
            cookies: None,
            referrer: None,
            user_agent: Some(
                "Mozilla/5.0 (Windows NT 10.0; rv:128.0) Gecko/20100101 Firefox/128.0".to_string(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

    fn map_with_all() -> HeaderMap {
        let mut m = HeaderMap::new();
        m.insert("cookie", HeaderValue::from_static("session=abc"));
        m.insert("referer", HeaderValue::from_static("https://example.com/"));
        m.insert("user-agent", HeaderValue::from_static("curl/8.0"));
        m
    }

    /// Round-trip: struct fields match the source map, then `to_header_map`
    /// reproduces exactly the three canonical lowercased headers.
    #[test]
    fn round_trip_from_and_to_header_map() {
        let source = map_with_all();
        let headers = RequestHeaders::from_header_map(&source);
        assert_eq!(headers.cookies.as_deref(), Some("session=abc"));
        assert_eq!(headers.referrer.as_deref(), Some("https://example.com/"));
        assert_eq!(headers.user_agent.as_deref(), Some("curl/8.0"));

        let out = headers.to_header_map().unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(
            out.get("cookie").unwrap(),
            HeaderValue::from_static("session=abc")
        );
        assert_eq!(
            out.get("referer").unwrap(),
            HeaderValue::from_static("https://example.com/")
        );
        assert_eq!(
            out.get("user-agent").unwrap(),
            HeaderValue::from_static("curl/8.0")
        );
    }

    /// All-`None` struct produces an empty map (no headers inserted).
    #[test]
    fn omission_produces_empty_map() {
        let headers = RequestHeaders {
            cookies: None,
            referrer: None,
            user_agent: None,
        };
        let out = headers.to_header_map().unwrap();
        assert_eq!(out.len(), 0);
    }

    /// Garbage from a browser extension is rejected, not injected.
    #[test]
    fn invalid_header_value_is_rejected() {
        let headers = RequestHeaders {
            cookies: None,
            referrer: None,
            user_agent: Some("bad\0header".to_string()),
        };
        assert!(headers.to_header_map().is_err());
    }

    /// Default: Firefox ESR user-agent, cookies/referrer None.
    #[test]
    fn default_values() {
        let d = RequestHeaders::default();
        assert_eq!(d.cookies, None);
        assert_eq!(d.referrer, None);
        assert_eq!(
            d.user_agent.as_deref(),
            Some("Mozilla/5.0 (Windows NT 10.0; rv:128.0) Gecko/20100101 Firefox/128.0")
        );
    }
}
