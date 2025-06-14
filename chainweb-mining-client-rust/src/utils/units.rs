//! Unit prefix parsing utilities
//! Supports SI prefixes (K, M, G, T, P, E, Z, Y) and binary prefixes (Ki, Mi, Gi, Ti, Pi, Ei, Zi, Yi)

use crate::error::{Error, Result};

/// Parse a number with optional unit prefix
/// Supports both SI prefixes (powers of 1000) and binary prefixes (powers of 1024)
pub fn parse_with_unit_prefix(input: &str) -> Result<f64> {
    let input = input.trim();
    
    if input.is_empty() {
        return Err(Error::config("Empty unit value"));
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
        .map_err(|_| Error::config(format!("Invalid number: {}", number_str)))?;

    if number < 0.0 {
        return Err(Error::config("Unit values cannot be negative"));
    }

    Ok(number * multiplier)
}

/// Parse unit multiplier from suffix string
fn parse_unit_multiplier(unit: &str) -> Result<f64> {
    match unit.trim().to_lowercase().as_str() {
        // No unit
        "" => Ok(1.0),
        
        // SI prefixes (decimal, powers of 1000)
        "da" => Ok(10.0),
        "h" => Ok(100.0),
        "k" => Ok(1_000.0),
        "m" => Ok(1_000_000.0),
        "g" => Ok(1_000_000_000.0),
        "t" => Ok(1_000_000_000_000.0),
        "p" => Ok(1_000_000_000_000_000.0),
        "e" => Ok(1_000_000_000_000_000_000.0),
        "z" => Ok(1_000_000_000_000_000_000_000.0),
        "y" => Ok(1_000_000_000_000_000_000_000_000.0),
        
        // Binary prefixes (powers of 1024)
        "ki" => Ok(1_024.0),
        "mi" => Ok(1_048_576.0), // 1024^2
        "gi" => Ok(1_073_741_824.0), // 1024^3
        "ti" => Ok(1_099_511_627_776.0), // 1024^4
        "pi" => Ok(1_125_899_906_842_624.0), // 1024^5
        "ei" => Ok(1_152_921_504_606_846_976.0), // 1024^6
        "zi" => Ok(1_180_591_620_717_411_303_424.0), // 1024^7
        "yi" => Ok(1_208_925_819_614_629_174_706_176.0), // 1024^8
        
        _ => Err(Error::config(format!("Unknown unit prefix: {}", unit))),
    }
}

/// Parse hash rate with unit prefixes
pub fn parse_hash_rate(input: &str) -> Result<f64> {
    parse_with_unit_prefix(input)
}

/// Parse timeout value with unit prefixes (assumes seconds if no unit)
pub fn parse_timeout_seconds(input: &str) -> Result<f64> {
    let value = parse_with_unit_prefix(input)?;
    
    // Convert to seconds if the result is reasonable for microseconds
    // (Haskell uses microseconds, we use seconds)
    if value > 10_000_000.0 {
        // Likely microseconds, convert to seconds
        Ok(value / 1_000_000.0)
    } else {
        // Likely already in seconds
        Ok(value)
    }
}

/// Parse memory size with unit prefixes
pub fn parse_memory_size(input: &str) -> Result<u64> {
    let value = parse_with_unit_prefix(input)?;
    
    if value > u64::MAX as f64 {
        return Err(Error::config("Memory size too large"));
    }
    
    Ok(value as u64)
}

/// Format a number with appropriate unit prefix for display
pub fn format_with_unit_prefix(mut value: f64, use_binary: bool) -> String {
    if value == 0.0 {
        return "0".to_string();
    }

    let units = if use_binary {
        &["", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "Zi", "Yi"]
    } else {
        &["", "K", "M", "G", "T", "P", "E", "Z", "Y"]
    };

    let base = if use_binary { 1024.0 } else { 1000.0 };
    let mut unit_index = 0;

    while value >= base && unit_index < units.len() - 1 {
        value /= base;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{:.0}", value)
    } else if value.fract() == 0.0 {
        format!("{:.0}{}", value, units[unit_index])
    } else {
        format!("{:.2}{}", value, units[unit_index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_numbers() {
        assert_eq!(parse_with_unit_prefix("100").unwrap(), 100.0);
        assert_eq!(parse_with_unit_prefix("42.5").unwrap(), 42.5);
        assert_eq!(parse_with_unit_prefix("0").unwrap(), 0.0);
    }

    #[test]
    fn test_parse_si_prefixes() {
        assert_eq!(parse_with_unit_prefix("1K").unwrap(), 1_000.0);
        assert_eq!(parse_with_unit_prefix("2.5M").unwrap(), 2_500_000.0);
        assert_eq!(parse_with_unit_prefix("3G").unwrap(), 3_000_000_000.0);
        assert_eq!(parse_with_unit_prefix("1T").unwrap(), 1_000_000_000_000.0);
    }

    #[test]
    fn test_parse_binary_prefixes() {
        assert_eq!(parse_with_unit_prefix("1Ki").unwrap(), 1_024.0);
        assert_eq!(parse_with_unit_prefix("2Mi").unwrap(), 2_097_152.0);
        assert_eq!(parse_with_unit_prefix("1Gi").unwrap(), 1_073_741_824.0);
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(parse_with_unit_prefix("1k").unwrap(), 1_000.0);
        assert_eq!(parse_with_unit_prefix("1ki").unwrap(), 1_024.0);
        assert_eq!(parse_with_unit_prefix("1Mi").unwrap(), 1_048_576.0);
    }

    #[test]
    fn test_whitespace_handling() {
        assert_eq!(parse_with_unit_prefix(" 100 K ").unwrap(), 100_000.0);
        assert_eq!(parse_with_unit_prefix("42 Mi").unwrap(), 44_040_192.0);
    }

    #[test]
    fn test_invalid_inputs() {
        assert!(parse_with_unit_prefix("").is_err());
        assert!(parse_with_unit_prefix("abc").is_err());
        assert!(parse_with_unit_prefix("100X").is_err());
        assert!(parse_with_unit_prefix("-100").is_err());
    }

    #[test]
    fn test_hash_rate_parsing() {
        assert_eq!(parse_hash_rate("1000").unwrap(), 1000.0);
        assert_eq!(parse_hash_rate("1.5K").unwrap(), 1500.0);
        assert_eq!(parse_hash_rate("2.5M").unwrap(), 2_500_000.0);
    }

    #[test]
    fn test_timeout_conversion() {
        // Small values are treated as seconds
        assert_eq!(parse_timeout_seconds("5").unwrap(), 5.0);
        assert_eq!(parse_timeout_seconds("30").unwrap(), 30.0);
        
        // Large values (>10M) are treated as microseconds and converted to seconds
        assert_eq!(parse_timeout_seconds("50000000").unwrap(), 50.0);  // 50M microseconds = 50 seconds
        assert_eq!(parse_timeout_seconds("30000000").unwrap(), 30.0);  // 30M microseconds = 30 seconds
        
        // Values under the threshold stay as-is
        assert_eq!(parse_timeout_seconds("5000000").unwrap(), 5000000.0);  // Stays as 5M (under 10M threshold)
    }

    #[test]
    fn test_format_with_unit_prefix() {
        assert_eq!(format_with_unit_prefix(100.0, false), "100");
        assert_eq!(format_with_unit_prefix(1500.0, false), "1.50K");
        assert_eq!(format_with_unit_prefix(2_000_000.0, false), "2M");  // Whole numbers don't show decimals
        
        assert_eq!(format_with_unit_prefix(1024.0, true), "1Ki");  // Whole numbers don't show decimals
        assert_eq!(format_with_unit_prefix(2_097_152.0, true), "2Mi");  // Whole numbers don't show decimals
    }

    #[test]
    fn test_memory_size_parsing() {
        assert_eq!(parse_memory_size("100").unwrap(), 100);
        assert_eq!(parse_memory_size("1Ki").unwrap(), 1024);
        assert_eq!(parse_memory_size("2Mi").unwrap(), 2_097_152);
    }
}