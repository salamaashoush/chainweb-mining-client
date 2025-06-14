//! Remote configuration loading tests
//!
//! Tests the HTTP/HTTPS configuration loading functionality

use chainweb_mining_client::config::Config;
use std::path::PathBuf;

#[test]
fn test_format_detection() {
    // Test JSON detection
    let json_content = r#"{"node": "api.chainweb.com", "publicKey": "test"}"#;
    let config = Config::from_contents(json_content, "test.json").unwrap();
    assert_eq!(config.node.url, "api.chainweb.com");

    // Test YAML detection
    let yaml_content = r#"
node: api.chainweb.com
publicKey: test
"#;
    let config = Config::from_contents(yaml_content, "test.yaml").unwrap();
    assert_eq!(config.node.url, "api.chainweb.com");

    // Test TOML detection (using flat format)
    let toml_content = r#"node = "api.chainweb.com"
publicKey = "test"
account = "k:test"
worker = "cpu"
"#;
    let config = Config::from_contents(toml_content, "test.toml").unwrap();
    assert_eq!(config.node.url, "api.chainweb.com");
}

#[test]
fn test_auto_format_detection() {
    // Test JSON auto-detection (starts with {)
    let json_content = r#"{"node": "api.chainweb.com", "publicKey": "test"}"#;
    let config = Config::from_contents(json_content, "config").unwrap();
    assert_eq!(config.node.url, "api.chainweb.com");

    // Test TOML auto-detection (flat format with =)
    let toml_content = r#"node = "api.chainweb.com"
publicKey = "test"
account = "k:test"
worker = "cpu"
"#;
    let config = Config::from_contents(toml_content, "config").unwrap();
    assert_eq!(config.node.url, "api.chainweb.com");

    // Test YAML auto-detection (default fallback)
    let yaml_content = r#"
node: api.chainweb.com
publicKey: test
"#;
    let config = Config::from_contents(yaml_content, "config").unwrap();
    assert_eq!(config.node.url, "api.chainweb.com");
}

#[test]
fn test_url_detection() {
    // Test URL detection in from_file
    let http_path = PathBuf::from("http://example.com/config.yaml");
    let https_path = PathBuf::from("https://example.com/config.json");
    let file_path = PathBuf::from("/etc/mining/config.toml");

    // These should be detected as URLs (though they'll fail to load in tests)
    assert!(Config::from_file(&http_path).is_err()); // Should try to load from URL
    assert!(Config::from_file(&https_path).is_err()); // Should try to load from URL

    // This should be detected as a file (and fail because it doesn't exist)
    let err = Config::from_file(&file_path).unwrap_err();
    assert!(err.to_string().contains("Failed to read config file"));
}

#[tokio::test]
async fn test_config_url_async_interface() {
    // Test the async interface with a mock server
    use mockito::Server;

    let mut server = Server::new_async().await;

    // Mock a YAML config response
    let yaml_config = r#"
node: api.chainweb.com
publicKey: test123
worker: cpu
logLevel: info
"#;

    let mock = server
        .mock("GET", "/config.yaml")
        .with_status(200)
        .with_header("content-type", "text/yaml")
        .with_body(yaml_config)
        .create_async()
        .await;

    let url = format!("{}/config.yaml", server.url());
    let config = Config::from_url_async(&url).await.unwrap();

    assert_eq!(config.node.url, "api.chainweb.com");
    assert_eq!(config.mining.public_key, "test123");
    assert_eq!(config.logging.level, "info");

    mock.assert_async().await;
}

#[tokio::test]
async fn test_config_url_json_response() {
    use mockito::Server;

    let mut server = Server::new_async().await;

    // Mock a JSON config response (flat format)
    let json_config = r#"{
    "node": "test.chainweb.com",
    "publicKey": "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234",
    "worker": "stratum",
    "stratumPort": 3333,
    "logLevel": "debug"
}"#;

    let mock = server
        .mock("GET", "/config.json")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(json_config)
        .create_async()
        .await;

    let url = format!("{}/config.json", server.url());
    let config = Config::from_url_async(&url).await.unwrap();

    assert_eq!(config.node.url, "test.chainweb.com");
    assert_eq!(
        config.mining.public_key,
        "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234"
    );
    assert_eq!(config.logging.level, "debug");

    mock.assert_async().await;
}

#[tokio::test]
async fn test_config_url_error_handling() {
    use mockito::Server;

    let mut server = Server::new_async().await;

    // Test 404 error
    let mock_404 = server
        .mock("GET", "/missing.yaml")
        .with_status(404)
        .create_async()
        .await;

    let url = format!("{}/missing.yaml", server.url());
    let result = Config::from_url_async(&url).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("HTTP error 404"));

    mock_404.assert_async().await;

    // Test 500 error
    let mock_500 = server
        .mock("GET", "/error.yaml")
        .with_status(500)
        .create_async()
        .await;

    let url = format!("{}/error.yaml", server.url());
    let result = Config::from_url_async(&url).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("HTTP error 500"));

    mock_500.assert_async().await;
}

#[tokio::test]
async fn test_config_url_invalid_content() {
    use mockito::Server;

    let mut server = Server::new_async().await;

    // Mock invalid YAML response
    let invalid_yaml = "invalid: yaml: content: [unclosed";

    let mock = server
        .mock("GET", "/invalid.yaml")
        .with_status(200)
        .with_body(invalid_yaml)
        .create_async()
        .await;

    let url = format!("{}/invalid.yaml", server.url());
    let result = Config::from_url_async(&url).await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Failed to parse YAML config")
    );

    mock.assert_async().await;
}

#[test]
fn test_config_blocking_interface() {
    // Test that the blocking interface works (even if it fails due to network)
    let result = Config::from_url("https://httpbin.org/status/404");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("HTTP error 404"));
}

// Test configuration cascade with remote configs
#[tokio::test]
async fn test_config_cascade_with_remote() {
    use mockito::Server;
    use std::io::Write;
    use tempfile::NamedTempFile;

    let mut server = Server::new_async().await;

    // Create a local config file
    let mut local_file = NamedTempFile::new().unwrap();
    writeln!(
        local_file,
        r#"node = "local.chainweb.com"
useTls = false
publicKey = "local_key"
account = "k:local_key"
worker = "cpu"
threadCount = 2
logLevel = "error""#
    )
    .unwrap();

    // Mock a remote config that will override some values
    let remote_config = r#"node = "remote.chainweb.com"
useTls = true
publicKey = "local_key"
account = "k:local_key"
worker = "cpu"
threadCount = 4
logLevel = "debug""#;

    let mock = server
        .mock("GET", "/override.toml")
        .with_status(200)
        .with_body(remote_config)
        .create_async()
        .await;

    // Load local config first
    let local_config = Config::from_file(&local_file.path().to_path_buf()).unwrap();
    assert_eq!(local_config.node.url, "local.chainweb.com");
    assert!(!local_config.node.use_tls);
    assert_eq!(local_config.logging.level, "error");

    // Load remote config
    let url = format!("{}/override.toml", server.url());
    let remote_config = Config::from_url_async(&url).await.unwrap();
    assert_eq!(remote_config.node.url, "remote.chainweb.com");
    assert!(remote_config.node.use_tls);
    assert_eq!(remote_config.logging.level, "debug");

    mock.assert_async().await;
}
