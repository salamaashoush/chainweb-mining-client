//! Difficulty and hash rate calculations

use super::Target;

/// Represents mining difficulty
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Difficulty(pub f64);

impl Difficulty {
    /// Create a new difficulty
    pub fn new(value: f64) -> Self {
        Self(value)
    }

    /// Convert to target
    pub fn to_target(&self) -> Target {
        Target::from_difficulty(self.0).unwrap_or_else(|_| Target([0xFFu8; 32]))
    }
}

impl From<Target> for Difficulty {
    fn from(target: Target) -> Self {
        Self(target.to_difficulty())
    }
}

/// Represents hash rate in hashes per second
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HashRate(pub f64);

impl HashRate {
    /// Create a new hash rate
    pub fn new(hashes_per_second: f64) -> Self {
        Self(hashes_per_second)
    }
}

/// Represents a time period in seconds
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Period(pub f64);

impl Period {
    /// Create a new period
    pub fn new(seconds: f64) -> Self {
        Self(seconds)
    }
}

/// Adjust difficulty based on estimated hash rate and target period
/// 
/// This matches the Haskell adjustDifficulty function:
/// - tolerance: dead band to reduce jitter (e.g., 0.25 for 25%)
/// - estimated_hash_rate: current hash rate estimate
/// - target_period: desired time between shares (e.g., 10 seconds)
/// - current_difficulty: current difficulty setting
/// 
/// Returns the new difficulty, or the current difficulty if within tolerance
pub fn adjust_difficulty(
    tolerance: f64,
    estimated_hash_rate: HashRate,
    target_period: Period,
    current_difficulty: Difficulty,
) -> Difficulty {
    let HashRate(hr) = estimated_hash_rate;
    let Period(tp) = target_period;
    let Difficulty(d) = current_difficulty;
    
    // Current period = difficulty / hash_rate
    let current_period = d / hr;
    
    // Check if adjustment is needed
    let deviation = (current_period - tp).abs() / tp;
    if deviation <= tolerance {
        return current_difficulty;
    }
    
    // Calculate new difficulty to achieve target period
    // new_difficulty = current_difficulty * target_period / current_period
    let new_difficulty = d * tp / current_period;
    
    // Ensure difficulty stays within reasonable bounds
    prune_difficulty(Difficulty(new_difficulty))
}

/// Ensure difficulty stays within reasonable bounds
fn prune_difficulty(difficulty: Difficulty) -> Difficulty {
    const MIN_DIFFICULTY: f64 = 1.0;
    const MAX_DIFFICULTY: f64 = 1e15; // Reasonable upper bound
    
    let Difficulty(d) = difficulty;
    Difficulty(d.max(MIN_DIFFICULTY).min(MAX_DIFFICULTY))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjust_difficulty_no_change() {
        // If hash rate matches target, no adjustment needed
        let tolerance = 0.25;
        let hash_rate = HashRate::new(1000.0);
        let target_period = Period::new(10.0);
        let current_difficulty = Difficulty::new(10000.0); // 10000 / 1000 = 10 seconds
        
        let new_difficulty = adjust_difficulty(tolerance, hash_rate, target_period, current_difficulty);
        assert_eq!(new_difficulty.0, current_difficulty.0);
    }

    #[test]
    fn test_adjust_difficulty_increase() {
        // Hash rate too high, need to increase difficulty
        let tolerance = 0.25;
        let hash_rate = HashRate::new(2000.0); // Doubled hash rate
        let target_period = Period::new(10.0);
        let current_difficulty = Difficulty::new(10000.0); // 10000 / 2000 = 5 seconds (too fast)
        
        let new_difficulty = adjust_difficulty(tolerance, hash_rate, target_period, current_difficulty);
        assert!(new_difficulty.0 > current_difficulty.0);
        assert!((new_difficulty.0 - 20000.0).abs() < 1.0); // Should roughly double
    }

    #[test]
    fn test_adjust_difficulty_decrease() {
        // Hash rate too low, need to decrease difficulty
        let tolerance = 0.25;
        let hash_rate = HashRate::new(500.0); // Half hash rate
        let target_period = Period::new(10.0);
        let current_difficulty = Difficulty::new(10000.0); // 10000 / 500 = 20 seconds (too slow)
        
        let new_difficulty = adjust_difficulty(tolerance, hash_rate, target_period, current_difficulty);
        assert!(new_difficulty.0 < current_difficulty.0);
        assert!((new_difficulty.0 - 5000.0).abs() < 1.0); // Should roughly halve
    }

    #[test]
    fn test_adjust_difficulty_within_tolerance() {
        // Small deviation within tolerance
        let tolerance = 0.25;
        let hash_rate = HashRate::new(1100.0); // 10% higher
        let target_period = Period::new(10.0);
        let current_difficulty = Difficulty::new(10000.0); // 10000 / 1100 ≈ 9.09 seconds
        
        let new_difficulty = adjust_difficulty(tolerance, hash_rate, target_period, current_difficulty);
        assert_eq!(new_difficulty.0, current_difficulty.0); // No change
    }
}