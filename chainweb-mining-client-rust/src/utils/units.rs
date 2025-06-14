//! Unit prefix parsing utilities
//! Supports hash rate parsing with SI and binary prefixes

use crate::error::{Error, Result};

/// Parse hash rate with unit prefixes (e.g., "1M", "100K", "1.5G", "2Ki", "3Mi")
pub fn parse_hash_rate(input: &str) -> Result<f64> {
    let input = input.trim();
    
    if input.is_empty() {
        return Err(Error::config("Empty hash rate value"));
    }

    // Try to find the unit suffix
    let (number_str, multiplier) = if let Some(pos) = input.find(|c: char| c.is_ascii_alphabetic()) {
        let (num, unit) = input.split_at(pos);
        let mult = parse_unit_multiplier(unit)?;
        (num, mult)
    } else {
        // No unit suffix, just a number
        (input, 1.0)
    };

    // Parse the numeric part
    let number: f64 = number_str.trim().parse()
        .map_err(|_| Error::config(format!("Invalid hash rate number: {}", number_str)))?;

    if number < 0.0 {
        return Err(Error::config("Hash rate cannot be negative"));
    }

    Ok(number * multiplier)
}

/// Parse unit multiplier from suffix string
fn parse_unit_multiplier(unit: &str) -> Result<f64> {
    match unit.trim().to_lowercase().as_str() {
        // No unit
        "" => Ok(1.0),
        
        // SI prefixes (decimal, powers of 1000)
        "k" => Ok(1_000.0),
        "m" => Ok(1_000_000.0),
        "g" => Ok(1_000_000_000.0),
        "t" => Ok(1_000_000_000_000.0),
        
        // Binary prefixes (powers of 1024)
        "ki" => Ok(1_024.0),
        "mi" => Ok(1_048_576.0), // 1024^2
        "gi" => Ok(1_073_741_824.0), // 1024^3
        "ti" => Ok(1_099_511_627_776.0), // 1024^4
        
        _ => Err(Error::config(format!("Unknown hash rate unit prefix: {}", unit))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_rate_parsing() {
        assert_eq!(parse_hash_rate("1000").unwrap(), 1000.0);
        assert_eq!(parse_hash_rate("1.5K").unwrap(), 1500.0);
        assert_eq!(parse_hash_rate("2.5M").unwrap(), 2_500_000.0);
        assert_eq!(parse_hash_rate("3G").unwrap(), 3_000_000_000.0);
        assert_eq!(parse_hash_rate("1T").unwrap(), 1_000_000_000_000.0);
    }

    #[test]
    fn test_hash_rate_binary_prefixes() {
        assert_eq!(parse_hash_rate("1Ki").unwrap(), 1_024.0);
        assert_eq!(parse_hash_rate("2Mi").unwrap(), 2_097_152.0);
        assert_eq!(parse_hash_rate("1Gi").unwrap(), 1_073_741_824.0);
    }

    #[test]
    fn test_hash_rate_case_insensitive() {
        assert_eq!(parse_hash_rate("1k").unwrap(), 1_000.0);
        assert_eq!(parse_hash_rate("1ki").unwrap(), 1_024.0);
        assert_eq!(parse_hash_rate("1Mi").unwrap(), 1_048_576.0);
    }

    #[test]
    fn test_hash_rate_invalid_inputs() {
        assert!(parse_hash_rate("").is_err());
        assert!(parse_hash_rate("abc").is_err());
        assert!(parse_hash_rate("100X").is_err());
        assert!(parse_hash_rate("-100").is_err());
    }
}