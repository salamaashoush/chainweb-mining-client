//! Unit prefix parsing utilities
//! Supports parsing of numeric values with SI and binary prefixes

use crate::error::{Error, Result};

/// Parse a numeric value with unit prefixes (e.g., "1M", "100K", "1.5G", "2Ki", "3Mi")
/// 
/// Supports all standard SI prefixes (K, M, G, T, P, E, Z, Y) and binary prefixes (Ki, Mi, Gi, Ti, Pi, Ei, Zi, Yi)
pub fn parse_with_unit(input: &str) -> Result<f64> {
    let input = input.trim();
    
    if input.is_empty() {
        return Err(Error::config("Empty value"));
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

    Ok(number * multiplier)
}

/// Parse hash rate with unit prefixes (e.g., "1M", "100K", "1.5G", "2Ki", "3Mi")
pub fn parse_hash_rate(input: &str) -> Result<f64> {
    let value = parse_with_unit(input)?;
    
    if value < 0.0 {
        return Err(Error::config("Hash rate cannot be negative"));
    }
    
    Ok(value)
}

/// Parse unit multiplier from suffix string
fn parse_unit_multiplier(unit: &str) -> Result<f64> {
    match unit.trim().to_lowercase().as_str() {
        // No unit
        "" => Ok(1.0),
        
        // SI prefixes (decimal, powers of 1000)
        "k" => Ok(1e3),   // kilo
        "m" => Ok(1e6),   // mega
        "g" => Ok(1e9),   // giga
        "t" => Ok(1e12),  // tera
        "p" => Ok(1e15),  // peta
        "e" => Ok(1e18),  // exa
        "z" => Ok(1e21),  // zetta
        "y" => Ok(1e24),  // yotta
        
        // Binary prefixes (powers of 1024)
        "ki" => Ok(1024f64.powi(1)),  // kibi
        "mi" => Ok(1024f64.powi(2)),  // mebi
        "gi" => Ok(1024f64.powi(3)),  // gibi
        "ti" => Ok(1024f64.powi(4)),  // tebi
        "pi" => Ok(1024f64.powi(5)),  // pebi
        "ei" => Ok(1024f64.powi(6)),  // exbi
        "zi" => Ok(1024f64.powi(7)),  // zebi
        "yi" => Ok(1024f64.powi(8)),  // yobi
        
        _ => Err(Error::config(format!("Unknown unit prefix: {}", unit))),
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

    #[test]
    fn test_parse_with_unit_si_prefixes() {
        assert_eq!(parse_with_unit("1K").unwrap(), 1e3);
        assert_eq!(parse_with_unit("2.5M").unwrap(), 2.5e6);
        assert_eq!(parse_with_unit("3G").unwrap(), 3e9);
        assert_eq!(parse_with_unit("4T").unwrap(), 4e12);
        assert_eq!(parse_with_unit("5P").unwrap(), 5e15);
        assert_eq!(parse_with_unit("6E").unwrap(), 6e18);
        assert_eq!(parse_with_unit("7Z").unwrap(), 7e21);
        assert_eq!(parse_with_unit("8Y").unwrap(), 8e24);
    }

    #[test]
    fn test_parse_with_unit_binary_prefixes() {
        assert_eq!(parse_with_unit("1Ki").unwrap(), 1024.0);
        assert_eq!(parse_with_unit("2Mi").unwrap(), 2.0 * 1024f64.powi(2));
        assert_eq!(parse_with_unit("3Gi").unwrap(), 3.0 * 1024f64.powi(3));
        assert_eq!(parse_with_unit("4Ti").unwrap(), 4.0 * 1024f64.powi(4));
        assert_eq!(parse_with_unit("5Pi").unwrap(), 5.0 * 1024f64.powi(5));
        assert_eq!(parse_with_unit("6Ei").unwrap(), 6.0 * 1024f64.powi(6));
        assert_eq!(parse_with_unit("7Zi").unwrap(), 7.0 * 1024f64.powi(7));
        assert_eq!(parse_with_unit("8Yi").unwrap(), 8.0 * 1024f64.powi(8));
    }

    #[test]
    fn test_parse_with_unit_decimal_values() {
        assert_eq!(parse_with_unit("1.5K").unwrap(), 1500.0);
        assert_eq!(parse_with_unit("0.5M").unwrap(), 500_000.0);
        assert_eq!(parse_with_unit("2.75G").unwrap(), 2_750_000_000.0);
    }

    #[test]
    fn test_parse_with_unit_no_suffix() {
        assert_eq!(parse_with_unit("1000").unwrap(), 1000.0);
        assert_eq!(parse_with_unit("42.5").unwrap(), 42.5);
        assert_eq!(parse_with_unit("0").unwrap(), 0.0);
    }
}