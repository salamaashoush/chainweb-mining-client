//! Hex encoding utilities for Stratum protocol
//!
//! Provides functions for encoding and decoding hex strings used in Stratum messages.

use crate::error::{Error, Result};

/// Encode bytes as a lowercase hex string
pub fn encode_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

/// Decode a hex string to bytes
pub fn decode_hex(hex_str: &str) -> Result<Vec<u8>> {
    hex::decode(hex_str).map_err(|e| Error::stratum(format!("Invalid hex string: {}", e)))
}

/// Encode bytes as a lowercase hex string with "0x" prefix
pub fn encode_hex_prefixed(bytes: &[u8]) -> String {
    format!("0x{}", encode_hex(bytes))
}

/// Decode a hex string that may have "0x" prefix
pub fn decode_hex_flexible(hex_str: &str) -> Result<Vec<u8>> {
    let cleaned = if hex_str.starts_with("0x") || hex_str.starts_with("0X") {
        &hex_str[2..]
    } else {
        hex_str
    };
    decode_hex(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let data = vec![0x00, 0x01, 0x02, 0xFF, 0xAB, 0xCD];
        let encoded = encode_hex(&data);
        let decoded = decode_hex(&encoded).unwrap();
        assert_eq!(data, decoded);
    }

    #[test]
    fn test_encode_hex_format() {
        let data = vec![0xAB, 0xCD, 0xEF];
        let encoded = encode_hex(&data);
        assert_eq!(encoded, "abcdef");
    }

    #[test]
    fn test_encode_hex_prefixed() {
        let data = vec![0xAB, 0xCD];
        let encoded = encode_hex_prefixed(&data);
        assert_eq!(encoded, "0xabcd");
    }

    #[test]
    fn test_decode_hex_case_insensitive() {
        let lower = "abcdef";
        let upper = "ABCDEF";
        let mixed = "AbCdEf";

        assert_eq!(decode_hex(lower).unwrap(), decode_hex(upper).unwrap());
        assert_eq!(decode_hex(lower).unwrap(), decode_hex(mixed).unwrap());
    }

    #[test]
    fn test_decode_hex_flexible() {
        let data = vec![0xAB, 0xCD];
        
        assert_eq!(decode_hex_flexible("abcd").unwrap(), data);
        assert_eq!(decode_hex_flexible("0xabcd").unwrap(), data);
        assert_eq!(decode_hex_flexible("0Xabcd").unwrap(), data);
    }

    #[test]
    fn test_decode_hex_invalid() {
        assert!(decode_hex("xyz").is_err());
        assert!(decode_hex("abcg").is_err());
        assert!(decode_hex("abc").is_err()); // Odd length
    }

    #[test]
    fn test_empty_data() {
        let empty: Vec<u8> = vec![];
        let encoded = encode_hex(&empty);
        assert_eq!(encoded, "");
        
        let decoded = decode_hex(&encoded).unwrap();
        assert_eq!(decoded, empty);
    }
}
