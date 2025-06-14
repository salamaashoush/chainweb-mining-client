//! Debug format detection
use chainweb_mining_client::config::Config;

#[test]
fn test_format_detection_debug() {
    let toml_content = r#"[node]
url = "test.chainweb.com"

[mining]
public_key = "test_key""#;

    println!("TOML content: {}", toml_content);
    println!("Trimmed: {}", toml_content.trim());

    let result = Config::from_contents(toml_content, "test");
    match result {
        Ok(_) => println!("Successfully parsed as auto-detected format"),
        Err(e) => println!("Failed to parse: {}", e),
    }

    // Try explicitly as TOML
    let result2 = Config::from_contents(toml_content, "test.toml");
    match result2 {
        Ok(_) => println!("Successfully parsed as TOML"),
        Err(e) => println!("Failed to parse as TOML: {}", e),
    }
}
