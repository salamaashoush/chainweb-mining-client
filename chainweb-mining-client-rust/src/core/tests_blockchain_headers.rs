//! Tests using real blockchain header data from test-headers.bin
//!
//! This module ports the Haskell test that validates checkTarget works
//! correctly on real Kadena blockchain headers.

use std::fs;
use crate::core::{Work, Target, constants::WORK_SIZE};
use crate::core::target::check_target;

/// Size of a complete header in the test file (matching Haskell implementation)
const HEADER_SIZE: usize = 318;

/// Extract target from work bytes at the correct offset
/// 
/// This matches the Haskell extractTarget function which reads target
/// from bytes 158-189 (32 bytes) in little-endian format
fn extract_target_from_work(work_bytes: &[u8]) -> Target {
    if work_bytes.len() < 190 {
        panic!("Work bytes too short to contain target");
    }
    
    // Target is at bytes 158-189 (32 bytes) in little-endian format
    let mut target_bytes = [0u8; 32];
    target_bytes.copy_from_slice(&work_bytes[158..190]);
    
    Target::from_le_bytes(target_bytes)
}

/// Load and parse test headers from the binary file
/// 
/// This matches the Haskell testHeaders function which reads 318-byte chunks
fn load_test_headers() -> Vec<Vec<u8>> {
    let test_data_path = "../test/data/test-headers.bin";
    let bytes = fs::read(test_data_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", test_data_path, e));
    
    let mut headers = Vec::new();
    let mut offset = 0;
    
    while offset + HEADER_SIZE <= bytes.len() {
        let header = bytes[offset..offset + HEADER_SIZE].to_vec();
        headers.push(header);
        offset += HEADER_SIZE;
    }
    
    if offset < bytes.len() {
        panic!("Incomplete header at end of file. Expected {} bytes, got {}", 
               HEADER_SIZE, bytes.len() - offset);
    }
    
    headers
}

/// Extract work (first 286 bytes) from each header
/// 
/// This matches the Haskell testWorks function
fn extract_test_works() -> Vec<Work> {
    let headers = load_test_headers();
    headers.into_iter()
        .map(|header| {
            let work_slice = &header[..WORK_SIZE];
            Work::from_slice(work_slice).expect("Header should contain valid work")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_headers_file_exists() {
        // Verify we can load the test file
        let headers = load_test_headers();
        assert!(!headers.is_empty(), "Should load at least one header");
        
        // Each header should be exactly 318 bytes
        for (i, header) in headers.iter().enumerate() {
            assert_eq!(header.len(), HEADER_SIZE, 
                      "Header {} should be {} bytes", i, HEADER_SIZE);
        }
    }

    #[test] 
    fn test_extract_works_from_headers() {
        // Verify we can extract 286-byte work from headers
        let works = extract_test_works();
        assert!(!works.is_empty(), "Should extract at least one work");
        
        // Each work should be exactly 286 bytes
        for (i, work) in works.iter().enumerate() {
            assert_eq!(work.as_bytes().len(), WORK_SIZE,
                      "Work {} should be {} bytes", i, WORK_SIZE);
        }
    }

    #[test]
    fn test_target_extraction() {
        // Verify target extraction works on at least one header
        let works = extract_test_works();
        if !works.is_empty() {
            let work = &works[0];
            let target = extract_target_from_work(work.as_bytes());
            
            // Target should not be all zeros (sanity check)
            assert_ne!(target.as_bytes(), &[0u8; 32], "Target should not be all zeros");
        }
    }

    /// Port of test_checkTarget_testHeaders from Haskell
    /// 
    /// This is the main test that validates checkTarget succeeds 
    /// for all real blockchain headers in the test file
    #[test]
    fn test_check_target_test_headers() {
        let works = extract_test_works();
        assert!(!works.is_empty(), "Should have test data to validate");
        
        for (i, work) in works.iter().enumerate() {
            let target = extract_target_from_work(work.as_bytes());
            
            // This is the core validation - each work should pass checkTarget
            let result = check_target(&target, work);
            assert!(result.is_ok(), "checkTarget failed for work {}: {:?}", i, result.err());
            assert!(result.unwrap(), "checkTarget returned false for work {}", i);
        }
        
        println!("Successfully validated {} blockchain headers", works.len());
    }
}