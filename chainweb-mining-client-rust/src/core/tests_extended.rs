//! Additional unit tests for core types

#[cfg(test)]
mod work_tests {
    use crate::core::{Nonce, Work, constants::*, work::WorkBuilder};

    #[test]
    fn test_work_builder_chain_id() {
        let work = WorkBuilder::new()
            .chain_id(5.into())
            .timestamp(1234567890)
            .nonce(Nonce::new(999))
            .build()
            .unwrap();

        // Verify chain ID is set correctly (first 2 bytes)
        let bytes = work.as_bytes();
        let chain_id = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(chain_id, 5);
    }

    #[test]
    fn test_work_timestamp_update() {
        let mut work = Work::from_bytes([0u8; WORK_SIZE]);
        let timestamp = 1234567890u64;

        work.update_timestamp(timestamp);

        // Verify timestamp is updated (at offset 8)
        let bytes = work.as_bytes();
        let stored_timestamp = u64::from_le_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]);
        assert_eq!(stored_timestamp, timestamp);
    }

    #[test]
    fn test_work_display() {
        let mut work = Work::from_bytes([0u8; WORK_SIZE]);
        work.set_nonce(Nonce::new(12345));

        let display = format!("{}", work);
        assert!(display.contains("12345"));
    }

    #[test]
    fn test_work_debug() {
        let work = Work::from_bytes([0x42u8; WORK_SIZE]);
        let debug = format!("{:?}", work);

        assert!(debug.contains("Work"));
        assert!(debug.contains("hex"));
        assert!(debug.contains("nonce"));
    }
}

#[cfg(test)]
mod target_tests {
    use crate::core::Target;

    #[test]
    fn test_target_difficulty_conversion() {
        // Test basic difficulty conversion
        let target = Target::from_difficulty(1.0).unwrap();
        let difficulty = target.to_difficulty();
        assert!((difficulty - 1.0).abs() < 0.1);

        // Test higher difficulty
        let target = Target::from_difficulty(1000.0).unwrap();
        let difficulty = target.to_difficulty();
        assert!((difficulty - 1000.0).abs() < 100.0);
    }

    #[test]
    fn test_target_from_invalid_difficulty() {
        assert!(Target::from_difficulty(0.0).is_err());
        assert!(Target::from_difficulty(-1.0).is_err());
    }

    #[test]
    fn test_target_display() {
        let target = Target::from_bytes([0x00; 32]);
        let display = target.to_string();
        assert_eq!(display, "0".repeat(64));
    }

    #[test]
    fn test_target_edge_cases() {
        // Max target (all 0xFF)
        let max_target = Target::from_bytes([0xFF; 32]);
        let max_hash = [0xFF; 32];
        assert!(!max_target.meets_target(&max_hash)); // Equal doesn't meet

        let almost_max = [0xFE; 32];
        assert!(max_target.meets_target(&almost_max));

        // Min target (all 0x00)
        let min_target = Target::from_bytes([0x00; 32]);
        let any_hash = [0x01; 32];
        assert!(!min_target.meets_target(&any_hash)); // Nothing meets zero target
    }
}

#[cfg(test)]
mod nonce_tests {
    use crate::core::Nonce;

    #[test]
    fn test_nonce_overflow() {
        let mut nonce = Nonce::new(u64::MAX - 1);
        nonce.increment();
        assert_eq!(nonce.value(), u64::MAX);

        nonce.increment();
        assert_eq!(nonce.value(), 0); // Wraps around
    }

    #[test]
    fn test_nonce_serialization() {
        let nonce = Nonce::new(0xDEADBEEF);
        let json = serde_json::to_string(&nonce).unwrap();
        assert_eq!(json, "3735928559");

        let deserialized: Nonce = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, nonce);
    }
}

#[cfg(test)]
mod chain_id_tests {
    use crate::core::ChainId;

    #[test]
    fn test_chain_id_bounds() {
        // Valid chain IDs for mainnet are 0-19
        for i in 0..20 {
            let chain_id = ChainId::new(i);
            assert_eq!(chain_id.value(), i);
        }
    }

    #[test]
    fn test_chain_id_equality() {
        let id1 = ChainId::new(5);
        let id2 = ChainId::new(5);
        let id3 = ChainId::new(6);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_chain_id_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ChainId::new(1));
        set.insert(ChainId::new(2));
        set.insert(ChainId::new(1)); // Duplicate

        assert_eq!(set.len(), 2);
    }
}
