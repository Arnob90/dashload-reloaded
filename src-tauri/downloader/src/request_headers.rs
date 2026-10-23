use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RequestHeaders {
    pub cookies: Option<String>,
    pub referrer: Option<String>,
    pub user_agent: Option<String>,
}
impl RequestHeaders {
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
}
