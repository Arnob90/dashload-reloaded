use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RequestHeaders {
    pub cookies: Option<String>,
    pub referrer: Option<String>,
    pub user_agent: Option<String>,
}
