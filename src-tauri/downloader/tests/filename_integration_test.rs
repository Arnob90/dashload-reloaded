// tests/filename_integration_test.rs
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
// Import the function you want to test from your library crate
use downloader::filename_getter::get_filename;
use log::info;

#[tokio::test]
#[test_log::test]
async fn test_filename_from_content_disposition_head() {
    // --- 1. Arrange ---
    // Start a background mock server on a random port.
    let mock_server = MockServer::start().await;

    // Program the mock server to respond to a specific request.
    Mock::given(method("HEAD")) // We expect a HEAD request
        .and(path("/download.zip"))
        .respond_with(
            ResponseTemplate::new(200) // Respond with 200 OK
                .insert_header(
                    "Content-Disposition",
                    "attachment; filename=\"cool_file.zip\"", // The crucial header
                ),
        )
        // Mount the mock definition on the server.
        .mount(&mock_server)
        .await;

    // --- 2. Act ---
    // Call our function, pointing it at the mock server's URI.
    let client = reqwest::Client::new();
    let url_to_test = format!("{}/download.zip", mock_server.uri());
    let result = get_filename(client, &url_to_test).await;

    // --- 3. Assert ---
    // Check that the result is what we expect.
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "cool_file.zip");
}

#[test_log::test(tokio::test)]
async fn test_filename_from_url_fallback() {
    // Test the case where Content-Disposition is missing.
    // --- 1. Arrange ---
    let mock_server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/files/another-file.txt"))
        .respond_with(ResponseTemplate::new(200)) // No Content-Disposition header
        .mount(&mock_server)
        .await;

    // --- 2. Act ---
    let client = reqwest::Client::new();
    let url_to_test = format!("{}/files/another-file.txt", mock_server.uri());
    let result = get_filename(client, &url_to_test).await;
    info!("Logging from url fallback");
    info!("result: {:#?}", result);

    // --- 3. Assert ---
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "another-file.txt");
}

#[test_log::test(tokio::test)]
async fn test_fallback_to_get_if_head_fails() {
    // Test the case where HEAD gives an error, but GET works.
    // --- 1. Arrange ---
    let mock_server = MockServer::start().await;

    // First, program the HEAD failure. 405 Method Not Allowed is a common one.
    Mock::given(method("HEAD"))
        .and(path("/download.zip"))
        .respond_with(ResponseTemplate::new(405))
        .mount(&mock_server)
        .await;

    // Now, program the GET success.
    Mock::given(method("GET"))
        .and(path("/download.zip"))
        .respond_with(ResponseTemplate::new(200).insert_header(
            "Content-Disposition",
            "attachment; filename=\"get_fallback.zip\"",
        ))
        .mount(&mock_server)
        .await;

    // --- 2. Act ---
    let client = reqwest::Client::new();
    let url_to_test = format!("{}/download.zip", mock_server.uri());
    let result = get_filename(client, &url_to_test).await;

    // --- 3. Assert ---
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "get_fallback.zip");
}

#[test_log::test(tokio::test)]
async fn test_error_when_no_filename_can_be_found() {
    // --- 1. Arrange ---
    let mock_server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/a/directory/")) // Note the trailing slash
        .respond_with(ResponseTemplate::new(200)) // No Content-Disposition
        .mount(&mock_server)
        .await;

    // --- 2. Act ---
    let client = reqwest::Client::new();
    let url_to_test = format!("{}/a/directory/", mock_server.uri());
    let result = get_filename(client, &url_to_test).await;

    // --- 3. Assert ---
    assert!(result.is_err());
    // `matches!` is a great way to check the specific error variant.
    assert!(matches!(
        result.unwrap_err(),
        downloader::filename_getter::FilepathGettingError::Filenameless
    ));
}
