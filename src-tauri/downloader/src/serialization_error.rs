use thiserror::Error;

#[derive(Error, Debug)]
pub enum DataSerializationError {
    #[error("Failed to open or close file: {0}")]
    IoError(String),
    #[error("Failed to deserialize: {0}")]
    DeserializationError(String),
}

impl From<tokio::io::Error> for DataSerializationError {
    fn from(value: tokio::io::Error) -> Self {
        Self::IoError(value.to_string())
    }
}
impl From<serde_json::Error> for DataSerializationError {
    fn from(value: serde_json::Error) -> Self {
        Self::DeserializationError(value.to_string())
    }
}
