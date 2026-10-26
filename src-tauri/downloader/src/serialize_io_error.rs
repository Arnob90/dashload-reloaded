// Custom serializer that converts std::io::Error into a string for Serde
/// Serializes a `std::io::Error` as a plain string for Serde compatibility.
pub fn serialize_io_error<S>(err: &std::io::Error, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&err.to_string())
}
