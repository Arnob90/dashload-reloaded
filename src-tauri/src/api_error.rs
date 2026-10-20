use serde::{Deserialize, Serialize};
use std::error::Error;
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError<T: Error> {
    pub message: String,
    #[serde(flatten)]
    pub error: T,
}

impl<T: Error> ApiError<T> {
    pub fn new(error: T) -> Self {
        ApiError {
            message: error.to_string(),
            error,
        }
    }
}
impl<T: Error> From<T> for ApiError<T> {
    fn from(error: T) -> Self {
        ApiError::new(error)
    }
}
