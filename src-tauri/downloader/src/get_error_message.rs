use std::fmt::Display;

use serde::Serialize;
use serde_json::{self, Value, json};

/// Converts a serializable error type into a structured JSON `Value`
/// containing both the original fields and a `"message"` field with the `Display` output.
pub fn get_error_message<T: Serialize + Display>(given: T) -> Result<Value, serde_json::Error> {
    let mut val = serde_json::to_value(&given)?;
    if let Value::Object(ref mut map) = val {
        map.insert("message".to_string(), Value::String(given.to_string()));
        Ok(val)
    } else {
        Ok(json!({
            "kind": val,
            "message": given.to_string()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download_run_error::DownloadRunError;

    /// Newtype that serializes as a bare JSON string (non-object value),
    /// exercising the second branch of `get_error_message`.
    #[derive(Serialize)]
    struct StringNewtype(String);
    impl Display for StringNewtype {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    #[test]
    fn object_shaped_value_gets_message_field() {
        // AlreadyRunning serializes as {"kind":"AlreadyRunning"} (an object).
        let err = DownloadRunError::AlreadyRunning;
        let val = get_error_message(err).unwrap();
        let obj = val.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert_eq!(obj.get("kind").unwrap(), "AlreadyRunning");
        assert_eq!(
            obj.get("message").unwrap(),
            "Download is already running"
        );
    }

    #[test]
    fn non_object_value_wraps_in_kind_and_message() {
        let val = StringNewtype("connection reset".to_string());
        let result = get_error_message(val).unwrap();
        assert_eq!(
            result,
            json!({"kind": "connection reset", "message": "connection reset"})
        );
    }
}
