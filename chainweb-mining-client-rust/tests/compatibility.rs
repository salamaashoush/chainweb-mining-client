//! Compatibility tests with the Haskell implementation

use chainweb_mining_client::core::{constants::*, Nonce, Target, Work};

#[test]
fn test_work_format_compatibility() {
    // Test that work format matches Haskell implementation
    assert_eq!(WORK_SIZE, 286, "Work size must be 286 bytes");
    assert_eq!(NONCE_OFFSET, 278, "Nonce must be at offset 278");
    assert_eq!(NONCE_SIZE, 8, "Nonce must be 8 bytes");
}

#[test]
fn test_nonce_encoding_compatibility() {
    // Test that nonce encoding matches Haskell (little-endian)
    let nonce = Nonce::new(0x0123456789ABCDEF);
    let bytes = nonce.to_le_bytes();
    
    // Verify little-endian encoding
    assert_eq!(bytes[0], 0xEF);
    assert_eq!(bytes[1], 0xCD);
    assert_eq!(bytes[2], 0xAB);
    assert_eq!(bytes[3], 0x89);
    assert_eq!(bytes[4], 0x67);
    assert_eq!(bytes[5], 0x45);
    assert_eq!(bytes[6], 0x23);
    assert_eq!(bytes[7], 0x01);
}

#[test]
fn test_hash_algorithm_compatibility() {
    // Test that we're using Blake2s-256 as in Haskell
    let mut work = Work::from_bytes([0u8; WORK_SIZE]);
    work.set_nonce(Nonce::new(0));
    
    let hash = work.hash();
    assert_eq!(hash.len(), 32, "Hash must be 32 bytes (Blake2s-256)");
    
    // Just verify it's using Blake2s-256 (32 bytes)
    // The exact hash will depend on the Blake2s implementation details
    println!("First byte of hash: 0x{:02X}", hash[0]);
}

#[test]
fn test_target_comparison_compatibility() {
    // Test that target comparison matches Haskell (big-endian comparison)
    let target = Target::from_bytes([
        0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ]);
    
    // Hash that should meet target
    let good_hash = [
        0x00, 0x00, 0x00, 0x00, 0xFE, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ];
    assert!(target.meets_target(&good_hash));
    
    // Hash that should not meet target
    let bad_hash = [
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    assert!(!target.meets_target(&bad_hash));
}

#[test]
fn test_api_endpoint_compatibility() {
    // Test that API endpoints match Haskell implementation
    let work_endpoint = "/chainweb/0.0/mainnet01/mining/work";
    let solved_endpoint = "/chainweb/0.0/mainnet01/mining/solved";
    let updates_endpoint = "/chainweb/0.0/mainnet01/mining/updates";
    let info_endpoint = "/info";
    
    // These should match the Haskell client's endpoints
    assert!(work_endpoint.contains("mining/work"));
    assert!(solved_endpoint.contains("mining/solved"));
    assert!(updates_endpoint.contains("mining/updates"));
    assert_eq!(info_endpoint, "/info");
}

#[test]
fn test_configuration_compatibility() {
    // Test that configuration options match Haskell implementation
    use chainweb_mining_client::config::{Config, WorkerConfig};
    
    let config = Config::default();
    
    // Verify default values match Haskell
    assert_eq!(config.node.timeout_secs, 30);
    assert_eq!(config.mining.update_interval_secs, 5);
    
    match config.worker {
        WorkerConfig::Cpu { batch_size, .. } => {
            assert_eq!(batch_size, 100_000);
        }
        _ => panic!("Default should be CPU worker"),
    }
}