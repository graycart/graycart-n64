//! VR4300 COP0 System Control Coprocessor, TLB control registers, ERET, traps, & exception vectoring.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapters 5 & 6
//!   via docs/01-cpu-r4300i.md · docs/DEVELOPMENT-PLAN.md §3.3 (CountClock)

use serde::{Deserialize, Serialize};

use crate::schedule::{CountClock, Scheduler};
use crate::trap::Trap;

use super::decode::Inst;
use super::{CpuException, InterpCpu};

/// VR4300 COP0 register file + sync-aware wrap-safe [`CountClock`].
///
/// Field names are stable across Wave B so `tlb.rs`, `cop1.rs`, and `cache.rs`
/// can read/write COP0 fields without touching `cop0.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cop0 {
    pub index: u32,
    pub random: u32,
    pub entry_lo0: u64,
    pub entry_lo1: u64,
    pub context: u64,
    pub page_mask: u32,
    pub wired: u32,
    pub bad_vaddr: u64,
    pub count_clock: CountClock,
    pub entry_hi: u64,
    pub status: u32,
    pub cause: u32,
    pub epc: u64,
    pub prid: u32,
    pub config: u32,
    pub ll_addr: u32,
    pub watch_lo: u32,
    pub watch_hi: u32,
    pub xcontext: u64,
    pub parity_error: u32,
    pub cache_error: u32,
    pub tag_lo: u32,
    pub tag_hi: u32,
    pub error_epc: u64,
    /// Open-bus / latch value returned for undefined COP0 registers.
    pub latch: u64,
}

impl Cop0 {
    /// VR4300 `PRId` constant (`Imp = 0x0B`, `Rev = 0x22`).
    pub const PRID_VR4300: u32 = 0x0000_0B22;
    /// Default reset `Status` (`BEV=1`, `ERL=1`, `CU0=1`, `CU1=1`, `FR=1` on N64 soft-boot compatibility).
    pub const RESET_STATUS: u32 = 0x3400_0000;
    /// Default reset `Config` (`EC=0`, `BE=1`, `K0=3` write-back cached).
    pub const RESET_CONFIG: u32 = 0x7006_E463;

    #[must_use]
    pub fn new() -> Self {
        Self {
            index: 0,
            random: 31,
            entry_lo0: 0,
            entry_lo1: 0,
            context: 0,
            page_mask: 0,
            wired: 0,
            bad_vaddr: 0,
            count_clock: CountClock::new(),
            entry_hi: 0,
            status: Self::RESET_STATUS,
            cause: 0,
            epc: 0,
            prid: Self::PRID_VR4300,
            config: Self::RESET_CONFIG,
            ll_addr: 0,
            watch_lo: 0,
            watch_hi: 0,
            xcontext: 0,
            parity_error: 0,
            cache_error: 0,
            tag_lo: 0,
            tag_hi: 0,
            error_epc: 0,
            latch: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// `Status.CU1` (bit 29): `true` if COP1 FPU is enabled.
    #[must_use]
    pub const fn cu1_enabled(&self) -> bool {
        (self.status & (1 << 29)) != 0
    }

    /// `Status.FR` (bit 26): `false` = 16 paired 64-bit / 32-bit FPU regs, `true` = 32 full 64-bit regs.
    #[must_use]
    pub const fn fr_mode(&self) -> bool {
        (self.status & (1 << 26)) != 0
    }

    /// `Status.BEV` (bit 22): bootstrap exception vectors (`0xBFC0_0200+`) vs normal (`0x8000_0000+`).
    #[must_use]
    pub const fn bev(&self) -> bool {
        (self.status & (1 << 22)) != 0
    }

    /// `Status.EXL` (bit 1): exception level.
    #[must_use]
    pub const fn exl(&self) -> bool {
        (self.status & (1 << 1)) != 0
    }

    /// `Status.ERL` (bit 2): error level.
    #[must_use]
    pub const fn erl(&self) -> bool {
        (self.status & (1 << 2)) != 0
    }

    /// `Status.IE` (bit 0): global interrupt enable.
    #[must_use]
    pub const fn ie(&self) -> bool {
        (self.status & 1) != 0
    }

    /// `Status.KX` (bit 7): 64-bit kernel addressing mode.
    #[must_use]
    pub const fn kx(&self) -> bool {
        (self.status & (1 << 7)) != 0
    }

    /// Current ASID (`EntryHi[7:0]`).
    #[must_use]
    pub const fn asid(&self) -> u8 {
        (self.entry_hi & 0xFF) as u8
    }

    /// Step `Random` down through `[Wired..=31]` on each instruction.
    pub fn tick_random(&mut self) {
        let wired = (self.wired & 0x1F).min(31);
        if self.random <= wired {
            self.random = 31;
        } else {
            self.random -= 1;
        }
    }

    /// Set or clear external MI interrupt line (`Cause.IP2`, bit 10).
    pub fn set_mi_interrupt(&mut self, asserted: bool) {
        if asserted {
            self.cause |= 1 << 10;
        } else {
            self.cause &= !(1 << 10);
        }
    }

    /// Synchronize `Cause.IP7` (bit 15) from [`CountClock::ip7_pending`].
    pub fn sync_timer_ip7(&mut self) {
        if self.count_clock.ip7_pending {
            self.cause |= 1 << 15;
        } else {
            self.cause &= !(1 << 15);
        }
    }

    /// Read COP0 register `reg` at `now_cpu_cycles`.
    #[must_use]
    pub fn read_reg(&self, reg: u8, now_cpu_cycles: u64) -> u64 {
        match reg {
            0 => u64::from(self.index),
            1 => u64::from(self.random),
            2 => self.entry_lo0,
            3 => self.entry_lo1,
            4 => self.context,
            5 => u64::from(self.page_mask),
            6 => u64::from(self.wired),
            8 => self.bad_vaddr,
            9 => u64::from(self.count_clock.read_count(now_cpu_cycles)),
            10 => self.entry_hi,
            11 => u64::from(self.count_clock.compare()),
            12 => u64::from(self.status),
            13 => u64::from(self.cause),
            14 => self.epc,
            15 => u64::from(self.prid),
            16 => u64::from(self.config),
            17 => u64::from(self.ll_addr),
            18 => u64::from(self.watch_lo),
            19 => u64::from(self.watch_hi),
            20 => self.xcontext,
            26 => u64::from(self.parity_error),
            27 => u64::from(self.cache_error),
            28 => u64::from(self.tag_lo),
            29 => u64::from(self.tag_hi),
            30 => self.error_epc,
            7 | 21..=25 | 31 => self.latch,
            _ => self.latch,
        }
    }

    /// Write COP0 register `reg` with `val`.
    pub fn write_reg(&mut self, reg: u8, val: u64, sched: &mut Scheduler) {
        self.latch = val;
        match reg {
            0 => {
                self.index = (self.index & 0x8000_0000) | ((val as u32) & 0x3F);
            }
            1 => {
                // Random is read-only from MTC0
            }
            2 => {
                self.entry_lo0 = val & 0x3FFF_FFFF;
            }
            3 => {
                self.entry_lo1 = val & 0x3FFF_FFFF;
            }
            4 => {
                self.context = (val & !0x007F_FFFF) | (self.context & 0x007F_FFFF);
            }
            5 => {
                self.page_mask = (val as u32) & 0x01FF_E000;
            }
            6 => {
                self.wired = (val as u32) & 0x3F;
                self.random = 31;
            }
            8 => {
                // BadVAddr is read-only
            }
            9 => {
                self.count_clock.write_count(val as u32, sched);
            }
            10 => {
                self.entry_hi = val & 0xC000_00FF_FFFF_E0FF;
            }
            11 => {
                self.count_clock.write_compare(val as u32, sched);
                self.cause &= !(1 << 15);
            }
            12 => {
                self.status = (val as u32) & 0xFF57_FFFF;
            }
            13 => {
                self.cause = (self.cause & !0x0300) | ((val as u32) & 0x0300);
            }
            14 => {
                self.epc = val;
            }
            15 => {
                // PRId is read-only
            }
            16 => {
                self.config = (self.config & !0x0F00_800F) | ((val as u32) & 0x0F00_800F);
            }
            17 => {
                self.ll_addr = val as u32;
            }
            18 => {
                self.watch_lo = (val as u32) & 0xFFFF_FFFB;
            }
            19 => {
                self.watch_hi = (val as u32) & 0xF;
            }
            20 => {
                self.xcontext =
                    (val & !0x0000_0001_FFFF_FFFF) | (self.xcontext & 0x0000_0001_FFFF_FFFF);
            }
            26 => {
                self.parity_error = (val as u32) & 0xFF;
            }
            27 => {
                // CacheError is read-only
            }
            28 => {
                self.tag_lo = (val as u32) & 0x0FFF_FFC0;
            }
            29 => {
                self.tag_hi = 0;
            }
            30 => {
                self.error_epc = val;
            }
            _ => {}
        }
    }
}

impl Default for Cop0 {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if an enabled, unmasked interrupt is currently pending.
#[must_use]
pub fn has_pending_interrupt(cop0: &Cop0) -> bool {
    if !cop0.ie() || cop0.exl() || cop0.erl() {
        return false;
    }
    let ip = (cop0.cause >> 8) & 0xFF;
    let im = (cop0.status >> 8) & 0xFF;
    (ip & im) != 0
}

/// Deliver an architectural VR4300 exception into COP0 and vector `cpu.pc`/`next_pc`.
pub fn deliver_exception(cpu: &mut InterpCpu, pc: u64, exc: CpuException) {
    cpu.set_ll_bit(false);
    let base: u64 = if cpu.cop0.bev() {
        0xFFFF_FFFF_BFC0_0200
    } else {
        0xFFFF_FFFF_8000_0000
    };
    let offset: u64 = if !cpu.cop0.exl() && exc.is_tlb_refill() {
        0x000
    } else {
        0x180
    };

    if !cpu.cop0.exl() {
        if cpu.in_delay_slot() {
            cpu.cop0.epc = pc.wrapping_sub(4);
            cpu.cop0.cause |= 1 << 31;
        } else {
            cpu.cop0.epc = pc;
            cpu.cop0.cause &= !(1 << 31);
        }
        cpu.cop0.status |= 1 << 1;
    }

    cpu.cop0.cause = (cpu.cop0.cause & !0x7C) | (u32::from(exc.exc_code()) << 2);

    if let CpuException::CoprocessorUnusable { unit } = exc {
        cpu.cop0.cause = (cpu.cop0.cause & !(3 << 28)) | (u32::from(unit & 3) << 28);
    }

    if let Some(vaddr) = exc.bad_vaddr() {
        cpu.cop0.bad_vaddr = vaddr;
        if exc.is_tlb_exception() {
            let bad_vpn2 = (vaddr >> 13) & 0x7_FFFF;
            cpu.cop0.context = (cpu.cop0.context & !0x007F_FFFF) | (bad_vpn2 << 4);

            let x_bad_vpn2 = (vaddr >> 13) & 0x07FF_FFFF;
            let x_r = (vaddr >> 62) & 3;
            cpu.cop0.xcontext =
                (cpu.cop0.xcontext & !0x0000_0001_FFFF_FFFF) | (x_bad_vpn2 << 4) | (x_r << 31);

            let vpn2_field = vaddr & 0x0000_00FF_FFFF_E000;
            let r = (vaddr >> 62) & 3;
            let asid = cpu.cop0.entry_hi & 0xFF;
            cpu.cop0.entry_hi = vpn2_field | (r << 62) | asid;
        }
    }

    cpu.set_pc(base.wrapping_add(offset));
}

/// Execute `MFC0`, `DMFC0`, `MTC0`, `DMTC0`.
pub fn exec_cop0_move(
    cpu: &mut InterpCpu,
    sched: &mut Scheduler,
    pc: u64,
    inst: Inst,
) -> Result<u64, Trap> {
    let rs = inst.rs() as u8;
    let rd = inst.rd() as u8;
    let rt = inst.rt();

    match rs {
        0x00 => {
            let now = sched.now_cpu_cycles();
            let val = cpu.cop0.read_reg(rd, now);
            let sign_ext = (val as i32 as i64) as u64;
            cpu.set_gpr(rt, sign_ext);
            Ok(1)
        }
        0x01 => {
            let now = sched.now_cpu_cycles();
            let val = cpu.cop0.read_reg(rd, now);
            cpu.set_gpr(rt, val);
            Ok(1)
        }
        0x04 => {
            let gpr_val = cpu.gpr(rt);
            let val = match rd {
                14 | 30 | 10 | 4 | 20 => (gpr_val as i32 as i64) as u64,
                _ => gpr_val,
            };
            cpu.cop0.write_reg(rd, val, sched);
            Ok(1)
        }
        0x05 => {
            let val = cpu.gpr(rt);
            cpu.cop0.write_reg(rd, val, sched);
            Ok(1)
        }
        _ => Err(Trap::UnimplementedOpcode {
            pc,
            word: inst.raw(),
        }),
    }
}

/// Execute `ERET`.
pub fn exec_eret(cpu: &mut InterpCpu, _pc: u64, _inst: Inst) -> Result<u64, Trap> {
    let target = if cpu.cop0.erl() {
        cpu.cop0.status &= !(1 << 2);
        cpu.cop0.error_epc
    } else {
        cpu.cop0.status &= !(1 << 1);
        cpu.cop0.epc
    };
    cpu.set_ll_bit(false);
    cpu.jump_immediate(target);
    Ok(1)
}

/// Execute `SPECIAL` trap/syscall/break instructions.
pub fn exec_special_trap(cpu: &mut InterpCpu, pc: u64, inst: Inst) -> Result<u64, Trap> {
    let funct = inst.funct();
    match funct {
        0x0C => Err(cpu.raise_exception(pc, CpuException::Syscall)),
        0x0D => Err(cpu.raise_exception(pc, CpuException::Breakpoint)),
        0x30 | 0x31 | 0x32 | 0x33 | 0x34 | 0x36 => {
            let rs_val = cpu.gpr(inst.rs());
            let rt_val = cpu.gpr(inst.rt());
            let cond = match funct {
                0x30 => (rs_val as i64) >= (rt_val as i64),
                0x31 => rs_val >= rt_val,
                0x32 => (rs_val as i64) < (rt_val as i64),
                0x33 => rs_val < rt_val,
                0x34 => rs_val == rt_val,
                0x36 => rs_val != rt_val,
                _ => false,
            };
            if cond {
                Err(cpu.raise_exception(pc, CpuException::Trap))
            } else {
                Ok(1)
            }
        }
        _ => Err(Trap::UnimplementedOpcode {
            pc,
            word: inst.raw(),
        }),
    }
}

/// Execute `REGIMM` immediate trap instructions (`TGEI`, `TGEIU`, `TLTI`, `TLTIU`, `TEQI`, `TNEI`).
///
/// Per MIPS III / VR4300 spec, the 16-bit immediate is sign-extended to 64 bits
/// before both signed and unsigned comparisons.
pub fn exec_regimm_trap(cpu: &mut InterpCpu, pc: u64, inst: Inst) -> Result<u64, Trap> {
    let rt = inst.rt() as u8;
    let rs_val = cpu.gpr(inst.rs());
    let simm16 = inst.simm16();
    let simm16_u = simm16 as u64;

    let cond = match rt {
        0x08 => (rs_val as i64) >= simm16,
        0x09 => rs_val >= simm16_u,
        0x0A => (rs_val as i64) < simm16,
        0x0B => rs_val < simm16_u,
        0x0C => rs_val == simm16_u,
        0x0E => rs_val != simm16_u,
        _ => {
            return Err(Trap::UnimplementedOpcode {
                pc,
                word: inst.raw(),
            })
        }
    };

    if cond {
        Err(cpu.raise_exception(pc, CpuException::Trap))
    } else {
        Ok(1)
    }
}

#[cfg(test)]
#[path = "cop0_tests.rs"]
mod tests;
