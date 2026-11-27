//! VR4300 architectural exception definitions (M1.0 seam; Wave B wires COP0 vectoring).
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 6 (Exceptions)
//!   via docs/01-cpu-r4300i.md

use serde::{Deserialize, Serialize};

/// VR4300 architectural exceptions with hardware `Cause.ExcCode` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CpuException {
    /// External / timer / software interrupt (`ExcCode = 0`).
    Interrupt,
    /// TLB modification exception (`ExcCode = 1`).
    TlbModification { bad_vaddr: u64 },
    /// TLB miss on load or instruction fetch (`ExcCode = 2`).
    TlbLoad { bad_vaddr: u64 },
    /// TLB miss on store (`ExcCode = 3`).
    TlbStore { bad_vaddr: u64 },
    /// Address error on load or instruction fetch (`ExcCode = 4`).
    AddressErrorLoad { bad_vaddr: u64 },
    /// Address error on store (`ExcCode = 5`).
    AddressErrorStore { bad_vaddr: u64 },
    /// System call (`ExcCode = 8`).
    Syscall,
    /// Breakpoint (`ExcCode = 9`).
    Breakpoint,
    /// Reserved instruction (`ExcCode = 10`).
    ReservedInstruction,
    /// Coprocessor unusable (`ExcCode = 11`).
    CoprocessorUnusable { unit: u8 },
    /// Signed integer arithmetic overflow (`ExcCode = 12`).
    ArithmeticOverflow,
    /// Trap instruction condition met (`ExcCode = 13`).
    Trap,
    /// Floating-point exception (`ExcCode = 15`).
    FloatingPoint,
}

impl CpuException {
    /// Canonical short name for diagnostic logs and traps.
    #[must_use]
    pub const fn short_name(self) -> &'static str {
        match self {
            Self::Interrupt => "Int",
            Self::TlbModification { .. } => "Mod",
            Self::TlbLoad { .. } => "TLBL",
            Self::TlbStore { .. } => "TLBS",
            Self::AddressErrorLoad { .. } => "AdEL",
            Self::AddressErrorStore { .. } => "AdES",
            Self::Syscall => "Sys",
            Self::Breakpoint => "Bp",
            Self::ReservedInstruction => "RI",
            Self::CoprocessorUnusable { .. } => "CpU",
            Self::ArithmeticOverflow => "Ov",
            Self::Trap => "Tr",
            Self::FloatingPoint => "FPE",
        }
    }

    /// 5-bit `Cause.ExcCode` encoding per VR4300 User's Manual Table 6-2.
    #[must_use]
    pub const fn exc_code(self) -> u8 {
        match self {
            Self::Interrupt => 0,
            Self::TlbModification { .. } => 1,
            Self::TlbLoad { .. } => 2,
            Self::TlbStore { .. } => 3,
            Self::AddressErrorLoad { .. } => 4,
            Self::AddressErrorStore { .. } => 5,
            Self::Syscall => 8,
            Self::Breakpoint => 9,
            Self::ReservedInstruction => 10,
            Self::CoprocessorUnusable { .. } => 11,
            Self::ArithmeticOverflow => 12,
            Self::Trap => 13,
            Self::FloatingPoint => 15,
        }
    }

    /// Associated faulting virtual address, if this exception populates `BadVAddr`.
    #[must_use]
    pub const fn bad_vaddr(self) -> Option<u64> {
        match self {
            Self::TlbModification { bad_vaddr }
            | Self::TlbLoad { bad_vaddr }
            | Self::TlbStore { bad_vaddr }
            | Self::AddressErrorLoad { bad_vaddr }
            | Self::AddressErrorStore { bad_vaddr } => Some(bad_vaddr),
            _ => None,
        }
    }
}
