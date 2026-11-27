//! Reality Signal Processor (RSP) backend trait and LLE interpreter shell.
//!
//! Cited: N64brew — Reality Signal Processor
//!   https://n64brew.dev/wiki/Reality_Signal_Processor · docs/03-rsp.md
//! Note: 4 KiB DMEM + 4 KiB IMEM allocated in M0; SU/DMA land in M3, VU in M4.

use serde::{Deserialize, Serialize};

use crate::bus::Bus;
use crate::trap::Trap;

/// Size of RSP Data Memory (4 KiB).
pub const DMEM_SIZE: usize = 4096;

/// Size of RSP Instruction Memory (4 KiB).
pub const IMEM_SIZE: usize = 4096;

/// Execution interface for RSP backends (`InterpRsp` is the LLE reference).
pub trait RspBackend {
    fn run(&mut self, bus: &mut Bus, max_cycles: u64) -> Result<u64, Trap>;
}

/// Pure-Rust LLE RSP interpreter shell (SU lands in M3, VU in M4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpRsp {
    /// 4 KiB DMEM (`0x0400_0000..=0x0400_0FFF`).
    pub dmem: Vec<u8>,
    /// 4 KiB IMEM (`0x0400_1000..=0x0400_1FFF`).
    pub imem: Vec<u8>,
    /// 12-bit RSP program counter (wraps inside 4 KiB IMEM).
    pub pc: u16,
    /// True while SP_STATUS HALT bit is set (halted on power-on).
    pub halted: bool,
}

impl InterpRsp {
    #[must_use]
    pub fn new() -> Self {
        Self {
            dmem: vec![0; DMEM_SIZE],
            imem: vec![0; IMEM_SIZE],
            pc: 0,
            halted: true,
        }
    }

    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        Err(Trap::UnimplementedRegister {
            device: "SP",
            offset,
            is_write: false,
        })
    }

    pub fn write_reg(&mut self, offset: u32, _value: u32) -> Result<(), Trap> {
        Err(Trap::UnimplementedRegister {
            device: "SP",
            offset,
            is_write: true,
        })
    }
}

impl Default for InterpRsp {
    fn default() -> Self {
        Self::new()
    }
}

impl RspBackend for InterpRsp {
    fn run(&mut self, _bus: &mut Bus, _max_cycles: u64) -> Result<u64, Trap> {
        if self.halted {
            return Ok(0);
        }
        Err(Trap::UnimplementedOpcode {
            pc: u64::from(self.pc),
            word: 0,
        })
    }
}
