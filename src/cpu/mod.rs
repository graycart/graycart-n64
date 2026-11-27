//! NEC VR4300 (MIPS III) CPU backend trait, interpreter state, and fetch/decode/execute loop.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapters 2, 5, 6, 7–9, 11, 16–17
//!   via docs/01-cpu-r4300i.md · docs/ORCHESTRATOR-HANDOFF.md §4 (M1 Wave A + Wave B)

pub mod alu;
pub mod branch;
pub mod cache;
pub mod cop0;
pub mod cop1;
pub mod decode;
pub mod exceptions;
pub mod load_store;
pub mod mmu;
pub mod mult_div;
pub mod shift;
pub mod tlb;

use serde::{Deserialize, Serialize};

use crate::bus::Bus;
use crate::schedule::Scheduler;
use crate::trap::Trap;

pub use cache::Caches;
pub use cop0::Cop0;
pub use cop1::Cop1;
pub use exceptions::CpuException;
pub use tlb::{Tlb, TlbEntry};

/// Execution interface for VR4300 backends (`InterpCpu` is the permanent accuracy reference).
pub trait CpuBackend {
    /// Execute a single instruction or slice step and return CPU cycles elapsed.
    fn step(&mut self, bus: &mut Bus, sched: &mut Scheduler) -> Result<u64, Trap>;
}

/// Read-only core snapshot for CLI diagnostics and future Machine Monitor UI (`docs/13`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuDebugSnapshot {
    pub pc: u64,
    pub next_pc: u64,
    pub in_delay_slot: bool,
    pub gpr: [u64; 32],
    pub hi: u64,
    pub lo: u64,
    pub ll_bit: bool,
    pub ll_addr: u32,
    pub status: u32,
    pub cause: u32,
    pub epc: u64,
    pub last_bad_vaddr: u64,
    pub fcr31: u32,
    pub instructions_executed: u64,
}

/// Transient branch disposition set by `branch::exec_*` or `cop0::exec_eret` during a single step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BranchAction {
    #[default]
    None,
    Taken(u64),
    NotTaken,
    NullifyDelaySlot,
    /// Immediate PC jump with NO delay slot (used by `ERET` and exception vectoring).
    ImmediateJump(u64),
}

/// Pure-Rust VR4300 interpreter (permanent CI accuracy oracle).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpCpu {
    /// Program counter of the current instruction (64-bit, sign-extended in 32-bit mode).
    pc: u64,
    /// Program counter of the next instruction (branch delay slot or sequential `pc + 4`).
    next_pc: u64,
    /// `true` while the instruction at `pc` is executing in a branch delay slot (`Cause.BD`).
    in_delay_slot: bool,
    /// 32 general-purpose 64-bit registers (`gpr[0]` is hardwired to 0).
    gpr: [u64; 32],
    /// Multiply/divide accumulator registers.
    hi: u64,
    lo: u64,
    /// Load-linked / store-conditional reservation flag (`LLBit`).
    ll_bit: bool,
    /// Physical address recorded by `LL`/`LLD` (`CP0 LLAddr` physical base).
    ll_addr: u32,
    /// COP0 System Control Coprocessor (including `CountClock`).
    pub cop0: Cop0,
    /// 32-entry fully associative joint TLB.
    pub tlb: Tlb,
    /// COP1 IEEE-754 FPU.
    pub cop1: Cop1,
    /// Functional 16 KiB I-cache & 8 KiB write-back D-cache.
    pub caches: Caches,
    /// Total committed instructions executed since reset.
    instructions_executed: u64,
    /// Transient branch action recorded during `execute_instruction`.
    #[serde(skip)]
    pending_branch: BranchAction,
    /// Last raised [`CpuException`] captured for architectural delivery inside `step()`.
    #[serde(skip)]
    pending_exception: Option<CpuException>,
}

impl InterpCpu {
    /// Cold reset vector in KSEG1 PIF ROM space (`0xFFFF_FFFF_BFC0_0000`).
    pub const RESET_PC: u64 = 0xFFFF_FFFF_BFC0_0000;

    /// Create a newly reset VR4300 at the KSEG1 cold reset vector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pc: Self::RESET_PC,
            next_pc: Self::RESET_PC.wrapping_add(4),
            in_delay_slot: false,
            gpr: [0; 32],
            hi: 0,
            lo: 0,
            ll_bit: false,
            ll_addr: 0,
            cop0: Cop0::new(),
            tlb: Tlb::new(),
            cop1: Cop1::new(),
            caches: Caches::new(),
            instructions_executed: 0,
            pending_branch: BranchAction::None,
            pending_exception: None,
        }
    }

    /// Reset architectural registers to power-on state.
    pub fn reset(&mut self) {
        self.pc = Self::RESET_PC;
        self.next_pc = Self::RESET_PC.wrapping_add(4);
        self.in_delay_slot = false;
        self.gpr = [0; 32];
        self.hi = 0;
        self.lo = 0;
        self.ll_bit = false;
        self.ll_addr = 0;
        self.cop0.reset();
        self.tlb.reset();
        self.cop1.reset();
        self.caches.reset();
        self.instructions_executed = 0;
        self.pending_branch = BranchAction::None;
        self.pending_exception = None;
    }

    #[must_use]
    pub fn pc(&self) -> u64 {
        self.pc
    }

    #[must_use]
    pub fn next_pc(&self) -> u64 {
        self.next_pc
    }

    #[must_use]
    pub fn in_delay_slot(&self) -> bool {
        self.in_delay_slot
    }

    /// Directly set `pc` (clears delay-slot pipeline and sets `next_pc = pc + 4`).
    pub fn set_pc(&mut self, pc: u64) {
        self.pc = pc;
        self.next_pc = pc.wrapping_add(4);
        self.in_delay_slot = false;
        self.pending_branch = BranchAction::None;
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

    #[must_use]
    pub fn hi(&self) -> u64 {
        self.hi
    }

    pub fn set_hi(&mut self, val: u64) {
        self.hi = val;
    }

    #[must_use]
    pub fn lo(&self) -> u64 {
        self.lo
    }

    pub fn set_lo(&mut self, val: u64) {
        self.lo = val;
    }

    #[must_use]
    pub fn ll_bit(&self) -> bool {
        self.ll_bit
    }

    pub fn set_ll_bit(&mut self, val: bool) {
        self.ll_bit = val;
    }

    #[must_use]
    pub fn ll_addr(&self) -> u32 {
        self.ll_addr
    }

    pub fn set_ll_addr(&mut self, pa: u32) {
        self.ll_addr = pa;
        self.cop0.ll_addr = pa >> 4;
    }

    #[must_use]
    pub fn last_bad_vaddr(&self) -> u64 {
        self.cop0.bad_vaddr
    }

    #[must_use]
    pub fn instructions_executed(&self) -> u64 {
        self.instructions_executed
    }

    /// Capture a read-only debug snapshot for CLI telemetry or Machine Monitor UI.
    #[must_use]
    pub fn debug_snapshot(&self) -> CpuDebugSnapshot {
        CpuDebugSnapshot {
            pc: self.pc,
            next_pc: self.next_pc,
            in_delay_slot: self.in_delay_slot,
            gpr: self.gpr,
            hi: self.hi,
            lo: self.lo,
            ll_bit: self.ll_bit,
            ll_addr: self.ll_addr,
            status: self.cop0.status,
            cause: self.cop0.cause,
            epc: self.cop0.epc,
            last_bad_vaddr: self.cop0.bad_vaddr,
            fcr31: self.cop1.fcr31,
            instructions_executed: self.instructions_executed,
        }
    }

    /// Schedule a taken branch/jump: delay slot at `next_pc` executes next, then jumps to `target`.
    pub fn take_branch(&mut self, target: u64) {
        self.pending_branch = BranchAction::Taken(target);
    }

    /// Mark a non-likely branch as not taken: delay slot still executes with `in_delay_slot = true`.
    pub fn not_taken_branch(&mut self) {
        self.pending_branch = BranchAction::NotTaken;
    }

    /// Nullify the delay slot for a not-taken Branch Likely instruction (`pc` skips to `next_pc + 4`).
    pub fn nullify_delay_slot(&mut self) {
        self.pending_branch = BranchAction::NullifyDelaySlot;
    }

    /// Immediately jump to `target` without executing a delay slot (used by `ERET`).
    pub fn jump_immediate(&mut self, target: u64) {
        self.pending_branch = BranchAction::ImmediateJump(target);
    }

    /// Surface an architectural VR4300 exception from an instruction executor.
    ///
    /// Records the [`CpuException`] so [`InterpCpu::step`] delivers it via
    /// [`cop0::deliver_exception`], while also returning [`Trap::CpuException`] so
    /// direct unit tests on `exec_*` can assert exact exception codes cleanly.
    pub fn raise_exception(&mut self, pc: u64, exc: CpuException) -> Trap {
        self.ll_bit = false;
        if let Some(vaddr) = exc.bad_vaddr() {
            self.cop0.bad_vaddr = vaddr;
        }
        self.pending_exception = Some(exc);
        Trap::CpuException {
            pc,
            code: exc.short_name(),
            bad_vaddr: exc.bad_vaddr(),
        }
    }
}

impl Default for InterpCpu {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuBackend for InterpCpu {
    fn step(&mut self, bus: &mut Bus, sched: &mut Scheduler) -> Result<u64, Trap> {
        let cur_pc = self.pc;
        let delay_slot_pc = self.next_pc;

        self.cop0.sync_timer_ip7();
        if cop0::has_pending_interrupt(&self.cop0) {
            cop0::deliver_exception(self, cur_pc, CpuException::Interrupt);
            return Ok(1);
        }

        let (pa, cache) = match mmu::translate_fetch_va(cur_pc, &self.tlb, &self.cop0) {
            Ok(pair) => pair,
            Err(exc) => {
                cop0::deliver_exception(self, cur_pc, exc);
                return Ok(1);
            }
        };
        let word = bus.read_u32(pa, cache)?;

        self.pending_branch = BranchAction::None;
        self.pending_exception = None;

        let cycles = match decode::execute_instruction(self, bus, sched, cur_pc, word) {
            Ok(c) => c,
            Err(Trap::CpuException { .. }) => {
                if let Some(exc) = self.pending_exception.take() {
                    cop0::deliver_exception(self, cur_pc, exc);
                    return Ok(1);
                }
                return Err(Trap::UnimplementedOpcode { pc: cur_pc, word });
            }
            Err(hard_trap) => return Err(hard_trap),
        };

        self.instructions_executed = self.instructions_executed.saturating_add(1);
        self.cop0.tick_random();

        match self.pending_branch {
            BranchAction::None => {
                self.pc = delay_slot_pc;
                self.next_pc = delay_slot_pc.wrapping_add(4);
                self.in_delay_slot = false;
            }
            BranchAction::Taken(target) => {
                self.pc = delay_slot_pc;
                self.next_pc = target;
                self.in_delay_slot = true;
            }
            BranchAction::NotTaken => {
                self.pc = delay_slot_pc;
                self.next_pc = delay_slot_pc.wrapping_add(4);
                self.in_delay_slot = true;
            }
            BranchAction::NullifyDelaySlot => {
                let resume = delay_slot_pc.wrapping_add(4);
                self.pc = resume;
                self.next_pc = resume.wrapping_add(4);
                self.in_delay_slot = false;
            }
            BranchAction::ImmediateJump(target) => {
                self.pc = target;
                self.next_pc = target.wrapping_add(4);
                self.in_delay_slot = false;
            }
        }

        Ok(cycles)
    }
}
