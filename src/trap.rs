//! Structured trap/fault types enforcing the "Trap, don't stub" rule.
//!
//! Cited: Graycart N64 Development Plan §5.1 — Trap, don't stub
//!   docs/DEVELOPMENT-PLAN.md
//! Cross-check: N64brew — Memory map (SysAD freeze conditions)
//!   https://n64brew.dev/wiki/Memory_map

use std::fmt;

/// Hardware freeze or unimplemented-path trap.
///
/// Unimplemented opcodes, registers, COP0 fields, RDP commands, and illegal
/// SysAD accesses MUST surface a `Trap` rather than silently returning `0` or
/// continuing as a NOP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trap {
    /// Access to an unmapped physical region that freezes retail N64 hardware.
    UnmappedFreeze { pa: u32, size: u8, is_write: bool },
    /// Cached CPU access outside RDRAM (freezes retail hardware).
    CachedMmioFreeze { pa: u32 },
    /// 64-bit read inside the RCP register window (second SysAD beat never arrives).
    Rcp64BitReadFreeze { pa: u32 },
    /// Misaligned memory access.
    MisalignedAccess { pa: u32, size: u8 },
    /// Unimplemented MMIO register access (explicit trap, never returns 0).
    UnimplementedRegister {
        device: &'static str,
        offset: u32,
        is_write: bool,
    },
    /// Unimplemented VR4300 or RSP instruction word.
    UnimplementedOpcode { pc: u64, word: u32 },
    /// Unimplemented COP0 register read or write.
    UnimplementedCop0Reg { pc: u64, reg: u8, is_write: bool },
    /// Unimplemented RDP command ID.
    UnimplementedRdpCommand { cmd_id: u8, word: u64 },
    /// Architectural VR4300 exception surfaced before Wave B COP0 exception vectoring lands.
    CpuException {
        pc: u64,
        code: &'static str,
        bad_vaddr: Option<u64>,
    },
}

impl fmt::Display for Trap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmappedFreeze { pa, size, is_write } => write!(
                f,
                "unmapped physical freeze ({}, {size}-byte) at PA 0x{pa:08X}",
                if *is_write { "write" } else { "read" }
            ),
            Self::CachedMmioFreeze { pa } => {
                write!(
                    f,
                    "cached access to non-RDRAM MMIO freezes at PA 0x{pa:08X}"
                )
            }
            Self::Rcp64BitReadFreeze { pa } => {
                write!(f, "64-bit read in RCP window freezes at PA 0x{pa:08X}")
            }
            Self::MisalignedAccess { pa, size } => {
                write!(f, "misaligned {size}-byte access at PA 0x{pa:08X}")
            }
            Self::UnimplementedRegister {
                device,
                offset,
                is_write,
            } => write!(
                f,
                "unimplemented {device} register {} at offset 0x{offset:04X}",
                if *is_write { "write" } else { "read" }
            ),
            Self::UnimplementedOpcode { pc, word } => {
                write!(f, "unimplemented opcode 0x{word:08X} at PC 0x{pc:016X}")
            }
            Self::UnimplementedCop0Reg { pc, reg, is_write } => write!(
                f,
                "unimplemented COP0 reg {reg} {} at PC 0x{pc:016X}",
                if *is_write { "write" } else { "read" }
            ),
            Self::UnimplementedRdpCommand { cmd_id, word } => write!(
                f,
                "unimplemented RDP command 0x{cmd_id:02X} (word 0x{word:016X})"
            ),
            Self::CpuException {
                pc,
                code,
                bad_vaddr: Some(vaddr),
            } => write!(
                f,
                "CPU exception {code} at PC 0x{pc:016X} (BadVAddr 0x{vaddr:016X})"
            ),
            Self::CpuException {
                pc,
                code,
                bad_vaddr: None,
            } => write!(f, "CPU exception {code} at PC 0x{pc:016X}"),
        }
    }
}

impl std::error::Error for Trap {}
