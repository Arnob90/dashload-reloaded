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
