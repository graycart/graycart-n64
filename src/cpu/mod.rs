//! NEC VR4300 (MIPS III) CPU backend trait and interpreter shell.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1)
//!   via docs/01-cpu-r4300i.md (M0 shell; full integer/COP0/TLB/COP1/cache land in M1).

use serde::{Deserialize, Serialize};

use crate::bus::Bus;
use crate::schedule::Scheduler;
use crate::trap::Trap;

/// Execution interface for VR4300 backends (`InterpCpu` is permanent accuracy reference).
pub trait CpuBackend {
    /// Execute a single instruction or slice step and return CPU cycles elapsed.
    fn step(&mut self, bus: &mut Bus, sched: &mut Scheduler) -> Result<u64, Trap>;
}

/// Pure-Rust VR4300 interpreter (scaffold shell for M0; M1 implements ISA/COP0/TLB/FPU/caches).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpCpu {
    /// Program counter (64-bit, sign-extended in 32-bit mode).
    pc: u64,
    /// 32 general-purpose 64-bit registers (`gpr[0]` is hardwired to 0).
    gpr: [u64; 32],
    /// Multiply/divide accumulator registers.
    hi: u64,
    lo: u64,
}

impl InterpCpu {
    /// Create a newly reset VR4300 at the KSEG1 cold reset vector (`0xBFC0_0000`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            pc: 0xFFFF_FFFF_BFC0_0000,
            gpr: [0; 32],
            hi: 0,
            lo: 0,
        }
    }

    /// Reset architectural registers to power-on state.
    pub fn reset(&mut self) {
        self.pc = 0xFFFF_FFFF_BFC0_0000;
        self.gpr = [0; 32];
        self.hi = 0;
        self.lo = 0;
    }

    #[must_use]
    pub fn pc(&self) -> u64 {
        self.pc
    }

    pub fn set_pc(&mut self, pc: u64) {
        self.pc = pc;
    }

    #[must_use]
    pub fn gpr(&self, idx: usize) -> u64 {
        if idx == 0 {
            0
        } else {
            self.gpr[idx]
        }
    }

    pub fn set_gpr(&mut self, idx: usize, val: u64) {
        if idx != 0 {
            self.gpr[idx] = val;
        }
    }
}

impl Default for InterpCpu {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuBackend for InterpCpu {
    fn step(&mut self, _bus: &mut Bus, _sched: &mut Scheduler) -> Result<u64, Trap> {
        // Trap, don't stub (§5.1): M1 replaces this with real fetch/decode/execute.
        Err(Trap::UnimplementedOpcode {
            pc: self.pc,
            word: 0,
        })
    }
}
