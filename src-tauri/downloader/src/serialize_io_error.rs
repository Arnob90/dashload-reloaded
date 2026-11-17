// Custom serializer that converts std::io::Error into a string for Serde
/// Serializes a `std::io::Error` as a plain string for Serde compatibility.
pub fn serialize_io_error<S>(err: &std::io::Error, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&err.to_string())
}

#[cfg(test)]
mod tests {
    use crate::verifier::VerifyError;

    /// `VerifyError::IOError` serializes the io::Error payload as the plain
    /// Display string (not the io::Error struct) under serde's default
    /// externally-tagged shape.
    #[test]
    fn io_error_serializes_as_string_payload() {
        let err =
            VerifyError::IOError(std::io::Error::new(std::io::ErrorKind::NotFound, "nope"));
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["IOError"], "nope");
    }
}
