# Missing Features in Rust Implementation

This document lists all features present in the Haskell implementation but missing from the Rust implementation of the chainweb mining client.

## Critical Features (High Priority)

### 1. Dynamic Difficulty Adjustment for Stratum
**Location**: `src/Worker/POW/Stratum/Server.hs`

The Haskell implementation supports three difficulty modes:
- `WorkDifficulty` - Uses the difficulty from the mining work
- `DifficultyLevel Int` - Fixed difficulty level
- `DifficultyPeriod Double` - **Dynamic adjustment based on hash rate**

The dynamic adjustment (`DifficultyPeriod`) is missing in Rust. This feature:
- Monitors the rate of share submissions from each miner
- Adjusts difficulty to maintain approximately 10-second intervals between shares
- Prevents fast miners from overwhelming the server
- Ensures slow miners still submit shares frequently enough

**Implementation needed**:
```rust
// In chainweb-mining-client-rust/src/workers/stratum/difficulty.rs
pub enum Difficulty {
    Work,                    // Use work difficulty (exists)
    Level(u32),             // Fixed level (exists) 
    Period(f64),            // Dynamic adjustment (MISSING)
}

// Need to implement:
// - Hash rate estimation based on share timestamps
// - Difficulty adjustment algorithm
// - Per-session difficulty tracking
```

### 2. Session-Level Hash Rate Tracking
**Location**: `src/Worker/POW/Stratum/Server.hs` (lines with `estimateHashRate`)

The Haskell implementation tracks hash rate per mining session to enable dynamic difficulty adjustment. This includes:
- Storing timestamps of recent shares
- Calculating hash rate based on share submission frequency
- Using hash rate to adjust difficulty

**Implementation needed**:
```rust
// In StratumSession
pub struct StratumSession {
    // ... existing fields ...
    recent_shares: VecDeque<Instant>,  // MISSING
    estimated_hashrate: f64,           // MISSING
    current_difficulty: u32,           // MISSING
}
```

## Important Features (Medium Priority)

### 3. Target Utility Functions
**Location**: `src/Target.hs`

Missing utility functions:
- `mkTargetLevel :: Natural -> Target` - Create target from difficulty level
- `getTargetLevel :: Target -> Maybe Int` - Extract difficulty level from target
- `leveled :: Target -> Target` - Round target to nearest power of 2
- `adjustDifficulty :: Double -> Natural -> HashRate -> Duration -> Natural` - Difficulty adjustment algorithm

### 4. Share Validation Against Session Target
**Location**: `src/Worker/POW/Stratum/Server.hs` (handleSubmit function)

The Haskell implementation validates submitted shares against the session-specific target (which may differ from work target due to difficulty adjustment). Rust currently has basic validation only.

### 5. Stratum Authorization Callback
**Location**: `src/Worker/POW/Stratum/Server.hs`

Haskell supports custom authorization logic via the `StratumConf`:
```haskell
_stratumAuthorize :: !(Text -> Text -> IO Bool)
```

Rust currently always accepts authorization requests.

### 6. Job Time Field Updates
**Location**: `src/Worker/POW/Stratum/Server.hs` (incrementJobTime function)

For long-running jobs, the Haskell implementation can increment the time field in the work header. This feature is used when miners exhaust the nonce space.

## Minor Features (Low Priority)

### 7. Colored Terminal Output
**Location**: `src/Logger.hs`

The Haskell logger supports colored output for different log levels using ANSI escape codes. Rust uses the tracing crate which handles this differently.

### 8. Log Tag Stacking
**Location**: `src/Logger.hs` (withLogTag function)

Haskell supports hierarchical log tags that can be stacked:
```haskell
withLogTag "stratum" $ withLogTag "session-123" $ logInfo "message"
-- Output: [stratum.session-123] message
```

Rust uses tracing spans which provide similar functionality but with different syntax.

### 9. Specific Error Response Codes
**Location**: `src/Worker/POW/Stratum/Protocol.hs`

Haskell defines specific error codes for Stratum responses:
- Job not found
- Duplicate share
- Low difficulty share
- Unauthorized worker
- Not subscribed

Rust returns generic error responses.

### 10. Custom External Worker Environment
**Location**: `src/Worker/External.hs`

While Rust supports environment variables for external workers, it doesn't have the exact same environment variable names and formatting as Haskell.

## Configuration Options

### 11. Exact Default Value Compatibility
Some default values differ slightly:
- Stratum interface: `"*"` (Haskell) vs `"0.0.0.0"` (Rust)
- HTTP timeout units: microseconds (Haskell) vs seconds (Rust)

## Testing Infrastructure

### 12. Expect Script for Stratum
**Location**: `scripts/stratum.expect`

Haskell includes an expect script for testing Stratum protocol interaction. Rust uses unit tests instead, which is actually more maintainable.

## Summary

The most critical missing feature is the **dynamic difficulty adjustment** for Stratum servers. This is essential for production use with miners of varying hash rates. The other missing features are either:
- Nice-to-have improvements (colored logging, tag stacking)
- Features with acceptable alternatives in Rust (error codes, testing approach)
- Minor compatibility issues (default values, environment variables)

To achieve full feature parity, implementing dynamic difficulty adjustment should be the top priority, followed by the target utility functions and enhanced share validation.