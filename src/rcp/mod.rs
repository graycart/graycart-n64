//! Shared Reality Coprocessor (RCP) clock and structural constants.
//!
//! Cited: N64brew — Architecture / Clock domains
//!   https://n64brew.dev/wiki/Memory_map · docs/00-architecture-overview.md

/// VR4300 CPU master clock (NTSC ~93.75 MHz).
pub const CPU_CLOCK_HZ_NTSC: u64 = 93_750_000;

/// Reality Coprocessor clock (NTSC ~62.5 MHz = CPU * 2 / 3).
pub const RCP_CLOCK_HZ_NTSC: u64 = 62_500_000;

/// Exact rational numerator for RCP-to-CPU clock ratio (2 / 3).
pub const RCP_RATIO_NUM: u64 = 2;

/// Exact rational denominator for RCP-to-CPU clock ratio (2 / 3).
pub const RCP_RATIO_DEN: u64 = 3;
