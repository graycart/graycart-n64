//! Reality Signal Processor (RSP) backend trait and LLE interpreter.
//!
//! Cited: N64brew — Reality Signal Processor
//!   https://n64brew.dev/wiki/Reality_Signal_Processor · docs/03-rsp.md
//! Note: 4 KiB DMEM + 4 KiB IMEM + SP MMIO/DMA live on `Bus` as the single source of truth;
//! SU executes in M3, VU (COP2) lands in M4.

pub mod regs;
pub mod scalar;

use serde::{Deserialize, Serialize};

use crate::bus::Bus;
use crate::trap::Trap;

pub use regs::{Sp, SpAction};

/// Size of RSP Data Memory (4 KiB).
pub const DMEM_SIZE: usize = 4096;

/// Size of RSP Instruction Memory (4 KiB).
pub const IMEM_SIZE: usize = 4096;

/// Execution interface for RSP backends (`InterpRsp` is the LLE reference).
pub trait RspBackend {
    fn run(&mut self, bus: &mut Bus, max_cycles: u64) -> Result<u64, Trap>;
}

/// Pure-Rust LLE RSP interpreter (SU in M3, VU in M4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpRsp {
    /// 32×32-bit Scalar Unit GPRs (`gpr[0]` is hardwired to `0`).
    pub gpr: [u32; 32],
    /// Pending branch delay-slot target (12-bit aligned IMEM offset), if active.
    pub next_pc: Option<u16>,
    /// Telemetry: total RSP instructions retired.
    pub instructions_executed: u64,
}

impl InterpRsp {
    #[must_use]
    pub fn new() -> Self {
        Self {
            gpr: [0; 32],
            next_pc: None,
            instructions_executed: 0,
        }
    }

    pub fn reset(&mut self) {
        self.gpr = [0; 32];
        self.next_pc = None;
        self.instructions_executed = 0;
    }

    #[must_use]
    #[inline]
    pub fn gpr(&self, idx: usize) -> u32 {
        if idx == 0 {
            0
        } else {
            self.gpr[idx & 31]
        }
    }

    #[inline]
    pub fn set_gpr(&mut self, idx: usize, val: u32) {
        let i = idx & 31;
        if i != 0 {
            self.gpr[i] = val;
        }
    }

    /// Fetch a big-endian 32-bit instruction word from 4 KiB IMEM (wrapping mod 4096).
    #[must_use]
    #[inline]
    pub fn fetch_imem(&self, bus: &Bus, addr: u16) -> u32 {
        let a = (addr as usize) & 0x0FFC;
        u32::from_be_bytes([
            bus.sp_imem[a],
            bus.sp_imem[a + 1],
            bus.sp_imem[a + 2],
            bus.sp_imem[a + 3],
        ])
    }

    /// Read a byte from 4 KiB DMEM (wrapping mod 4096).
    #[must_use]
    #[inline]
    pub fn read_dmem_u8(&self, bus: &Bus, addr: u32) -> u8 {
        bus.sp_dmem[(addr as usize) & 0x0FFF]
    }

    /// Read a big-endian 16-bit halfword from 4 KiB DMEM (unaligned allowed, wraps byte-by-byte).
    #[must_use]
    #[inline]
    pub fn read_dmem_u16(&self, bus: &Bus, addr: u32) -> u16 {
        let b0 = self.read_dmem_u8(bus, addr);
        let b1 = self.read_dmem_u8(bus, addr.wrapping_add(1));
        u16::from_be_bytes([b0, b1])
    }

    /// Read a big-endian 32-bit word from 4 KiB DMEM (unaligned allowed, wraps byte-by-byte).
    #[must_use]
    #[inline]
    pub fn read_dmem_u32(&self, bus: &Bus, addr: u32) -> u32 {
        let b0 = self.read_dmem_u8(bus, addr);
        let b1 = self.read_dmem_u8(bus, addr.wrapping_add(1));
        let b2 = self.read_dmem_u8(bus, addr.wrapping_add(2));
        let b3 = self.read_dmem_u8(bus, addr.wrapping_add(3));
        u32::from_be_bytes([b0, b1, b2, b3])
    }

    /// Write a byte to 4 KiB DMEM (wrapping mod 4096).
    #[inline]
    pub fn write_dmem_u8(&self, bus: &mut Bus, addr: u32, val: u8) {
        bus.sp_dmem[(addr as usize) & 0x0FFF] = val;
    }

    /// Write a big-endian 16-bit halfword to 4 KiB DMEM (unaligned allowed, wraps byte-by-byte).
    #[inline]
    pub fn write_dmem_u16(&self, bus: &mut Bus, addr: u32, val: u16) {
        let bytes = val.to_be_bytes();
        self.write_dmem_u8(bus, addr, bytes[0]);
        self.write_dmem_u8(bus, addr.wrapping_add(1), bytes[1]);
    }

    /// Write a big-endian 32-bit word to 4 KiB DMEM (unaligned allowed, wraps byte-by-byte).
    #[inline]
    pub fn write_dmem_u32(&self, bus: &mut Bus, addr: u32, val: u32) {
        let bytes = val.to_be_bytes();
        self.write_dmem_u8(bus, addr, bytes[0]);
        self.write_dmem_u8(bus, addr.wrapping_add(1), bytes[1]);
        self.write_dmem_u8(bus, addr.wrapping_add(2), bytes[2]);
        self.write_dmem_u8(bus, addr.wrapping_add(3), bytes[3]);
    }

    /// Execute one RSP Scalar instruction.
    pub fn step(&mut self, bus: &mut Bus) -> Result<u64, Trap> {
        let cycles = scalar::step_scalar(self, bus)?;
        self.instructions_executed = self.instructions_executed.saturating_add(1);
        if bus.sp.is_single_step() {
            bus.sp.status |= Sp::STATUS_HALT;
        }
        Ok(cycles)
    }
}

impl Default for InterpRsp {
    fn default() -> Self {
        Self::new()
    }
}

impl RspBackend for InterpRsp {
    fn run(&mut self, bus: &mut Bus, max_cycles: u64) -> Result<u64, Trap> {
        let mut elapsed = 0u64;
        while elapsed < max_cycles && !bus.sp.is_halted() {
            let c = self.step(bus)?;
            elapsed = elapsed.saturating_add(c.max(1));
        }
        Ok(elapsed)
    }
}
