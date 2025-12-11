pub fn get_err_string_for_multi_errors<ErrT: std::error::Error>(errors: &[ErrT]) -> String {
    errors
        .iter()
        .map(|err| err.to_string())
        .collect::<Vec<String>>()
        .join("\n")
}
