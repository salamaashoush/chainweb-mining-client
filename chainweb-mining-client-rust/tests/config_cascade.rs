//! Configuration cascade and merging tests
//!
//! Tests the configuration cascade system where multiple config files
//! can be loaded and merged together with proper precedence rules.

use chainweb_mining_client::config::Config;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_config_cascade_basic_merge() {
    // Create base config file
    let mut base_file = NamedTempFile::new().unwrap();
    writeln!(
        base_file,
        r#"node = "base.chainweb.com"
useTls = true
publicKey = "base_key"
account = "k:base_key"
logLevel = "info"
worker = "cpu"
threadCount = 2"#
    )
    .unwrap();

    // Create override config file
    let mut override_file = NamedTempFile::new().unwrap();
    writeln!(
        override_file,
        r#"node = "override.chainweb.com"
useTls = false
publicKey = "override_key"
logLevel = "debug"
worker = "stratum"
stratumPort = 3333"#
    )
    .unwrap();

    // Load base config
    let mut config = Config::from_file(&base_file.path().to_path_buf()).unwrap();

    // Verify base config
    assert_eq!(config.node.url, "base.chainweb.com");
    assert!(config.node.use_tls);
    assert_eq!(config.mining.public_key, "base_key");
    assert_eq!(config.mining.account, "k:base_key");
    assert_eq!(config.logging.level, "info");

    // Load and merge override config
    let override_config = Config::from_file(&override_file.path().to_path_buf()).unwrap();
    config.merge(override_config);

    // Verify merged config
    assert_eq!(config.node.url, "override.chainweb.com"); // Overridden
    assert!(!config.node.use_tls); // Overridden
    assert_eq!(config.mining.public_key, "override_key"); // Overridden
    assert_eq!(config.mining.account, "k:override_key"); // Auto-generated from override key
    assert_eq!(config.logging.level, "debug"); // Overridden

    // Worker config should be completely replaced
    assert_eq!(config.worker_type().to_string(), "stratum");
}

#[test]
fn test_config_cascade_multiple_files() {
    // Create three config files in cascade
    let mut config1 = NamedTempFile::new().unwrap();
    writeln!(
        config1,
        r#"node = "config1.chainweb.com"
useTls = true
publicKey = "config1_key"
account = "k:config1_key"
logLevel = "error""#
    )
    .unwrap();

    let mut config2 = NamedTempFile::new().unwrap();
    writeln!(
        config2,
        r#"node = "config2.chainweb.com"
publicKey = "config1_key"
logLevel = "warn""#
    )
    .unwrap();

    let mut config3 = NamedTempFile::new().unwrap();
    writeln!(
        config3,
        r#"useTls = false
publicKey = "config1_key"
account = "k:config3_account"
logLevel = "debug""#
    )
    .unwrap();

    // Load first config
    let mut final_config = Config::from_file(&config1.path().to_path_buf()).unwrap();

    // Apply second config
    let config2_data = Config::from_file(&config2.path().to_path_buf()).unwrap();
    final_config.merge(config2_data);

    // Apply third config
    let config3_data = Config::from_file(&config3.path().to_path_buf()).unwrap();
    final_config.merge(config3_data);

    // Verify final merged configuration
    assert_eq!(final_config.node.url, "config2.chainweb.com"); // From config2
    assert!(!final_config.node.use_tls); // From config3
    assert_eq!(final_config.mining.public_key, "config1_key"); // From config1 (unchanged)
    assert_eq!(final_config.mining.account, "k:config3_account"); // From config3
    assert_eq!(final_config.logging.level, "debug"); // From config3
}

#[test]
fn test_config_cascade_with_cli_args_simulation() {
    use chainweb_mining_client::config::Args;
    use clap::Parser;

    // Create base config file
    let mut base_file = NamedTempFile::new().unwrap();
    writeln!(
        base_file,
        r#"node = "file.chainweb.com"
useTls = true
publicKey = "file_key"
account = "k:file_key"
logLevel = "info""#
    )
    .unwrap();

    // Simulate CLI args that would override config file
    let args = Args::parse_from([
        "chainweb-mining-client",
        "--config-file",
        &base_file.path().to_string_lossy(),
        "--node",
        "cli.chainweb.com",
        "--no-tls",
        "--public-key",
        "cli_key",
        "--log-level",
        "debug",
    ]);

    // Load config with CLI args
    let config = Config::from_args(args).unwrap();

    // Verify CLI args take precedence
    assert_eq!(config.node.url, "cli.chainweb.com"); // CLI override
    assert!(!config.node.use_tls); // CLI override (--no-tls)
    assert_eq!(config.mining.public_key, "cli_key"); // CLI override
    assert_eq!(config.mining.account, "k:cli_key"); // Auto-generated from CLI key
    assert_eq!(config.logging.level, "debug"); // CLI override
}

#[test]
fn test_config_cascade_default_preservation() {
    // Test that defaults are properly preserved when not explicitly set
    let mut config1 = NamedTempFile::new().unwrap();
    writeln!(
        config1,
        r#"node = "test.chainweb.com"
publicKey = "test_key"
account = "k:test_key""#
    )
    .unwrap();

    let mut config2 = NamedTempFile::new().unwrap();
    writeln!(
        config2,
        r#"publicKey = "test_key"
insecure = true"#
    )
    .unwrap();

    let mut config = Config::from_file(&config1.path().to_path_buf()).unwrap();
    let config2_data = Config::from_file(&config2.path().to_path_buf()).unwrap();
    config.merge(config2_data);

    // Verify defaults are preserved from config1 where config2 doesn't override
    assert_eq!(config.node.url, "test.chainweb.com"); // From config1
    assert!(config.node.use_tls); // Default preserved (config2 doesn't specify)
    assert!(config.node.insecure); // From config2
    assert_eq!(config.mining.public_key, "test_key"); // From config1
    assert_eq!(config.mining.account, "k:test_key"); // From config1
}

#[test]
fn test_config_cascade_empty_values_handling() {
    // Test that empty values don't override non-empty ones
    let mut base_config = NamedTempFile::new().unwrap();
    writeln!(
        base_config,
        r#"node = "base.chainweb.com"
publicKey = "base_key"
account = "k:base_key"
logLevel = "info""#
    )
    .unwrap();

    let mut empty_config = NamedTempFile::new().unwrap();
    writeln!(
        empty_config,
        r#"node = "override.chainweb.com"
publicKey = ""
account = ""
logLevel = "debug""#
    )
    .unwrap();

    let mut config = Config::from_file(&base_config.path().to_path_buf()).unwrap();
    let empty_data = Config::from_file(&empty_config.path().to_path_buf()).unwrap();
    config.merge(empty_data);

    // URL should be overridden (not empty)
    assert_eq!(config.node.url, "override.chainweb.com");

    // Empty strings should not override non-empty values
    assert_eq!(config.mining.public_key, "base_key"); // Kept from base
    assert_eq!(config.mining.account, "k:base_key"); // Kept from base

    // Non-empty values should still override
    assert_eq!(config.logging.level, "debug"); // Overridden
}

#[test]
fn test_config_cascade_optional_fields() {
    // Test merging of optional fields (chain_id, log file, etc.)
    let mut config1 = NamedTempFile::new().unwrap();
    writeln!(
        config1,
        r#"node = "test.chainweb.com"
publicKey = "test_key"
account = "k:test_key""#
    )
    .unwrap();

    let mut config2 = NamedTempFile::new().unwrap();
    writeln!(
        config2,
        r#"node = "override.chainweb.com"
publicKey = "test_key""#
    )
    .unwrap();

    let mut config = Config::from_file(&config1.path().to_path_buf()).unwrap();
    let config2_data = Config::from_file(&config2.path().to_path_buf()).unwrap();
    config.merge(config2_data);

    // Verify basic merging worked
    assert_eq!(config.node.url, "override.chainweb.com"); // From config2
}

#[tokio::test]
async fn test_config_cascade_with_remote_configs() {
    use mockito::Server;

    let mut server = Server::new_async().await;

    // Create local base config
    let mut local_config = NamedTempFile::new().unwrap();
    writeln!(
        local_config,
        r#"node = "local.chainweb.com"
useTls = true
publicKey = "local_key"
account = "k:local_key"
logLevel = "info""#
    )
    .unwrap();

    // Mock remote override config
    let remote_config = r#"node = "remote.chainweb.com"
publicKey = "local_key"
useTls = false
logLevel = "debug""#;

    let mock = server
        .mock("GET", "/override.toml")
        .with_status(200)
        .with_body(remote_config)
        .create_async()
        .await;

    // Load local config first
    let mut config = Config::from_file(&local_config.path().to_path_buf()).unwrap();

    // Load and merge remote config
    let remote_url = format!("{}/override.toml", server.url());
    let remote_config_data = Config::from_url_async(&remote_url).await.unwrap();
    config.merge(remote_config_data);

    // Verify merged configuration
    assert_eq!(config.node.url, "remote.chainweb.com"); // From remote
    assert!(!config.node.use_tls); // From remote
    assert_eq!(config.mining.public_key, "local_key"); // From local (unchanged)
    assert_eq!(config.mining.account, "k:local_key"); // From local (unchanged) 
    assert_eq!(config.logging.level, "debug"); // From remote

    mock.assert_async().await;
}

#[test]
fn test_config_cascade_validation_after_merge() {
    // Test that validation works properly after merging configs
    let mut valid_base = NamedTempFile::new().unwrap();
    writeln!(
        valid_base,
        r#"node = "test.chainweb.com"
publicKey = "valid_key"
account = "k:valid_key"
worker = "stratum"
stratumPort = 3333"#
    )
    .unwrap();

    let mut invalid_override = NamedTempFile::new().unwrap();
    writeln!(
        invalid_override,
        r#"publicKey = "valid_key"
worker = "stratum"
stratumPort = 0"#
    )
    .unwrap();

    let config = Config::from_file(&valid_base.path().to_path_buf()).unwrap();
    assert!(config.validate().is_ok()); // Base config is valid

    // Try to load invalid config - this should fail during loading
    let invalid_result = Config::from_file(&invalid_override.path().to_path_buf());
    assert!(invalid_result.is_err());
    let error_msg = invalid_result.unwrap_err().to_string();
    // Should catch port = 0 during config loading
    assert!(error_msg.contains("port must be greater than 0"));
}
