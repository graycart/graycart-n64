//! VR4300 L1 I-cache (16 KiB) & D-cache (8 KiB) + `CACHE` instruction.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 11 (Caches)
//!   via docs/01-cpu-r4300i.md · docs/ORCHESTRATOR-HANDOFF.md §5 (DMA does NOT snoop D-cache)

use serde::{Deserialize, Serialize};

use crate::bus::Bus;
use crate::trap::Trap;

use super::decode::Inst;
use super::mmu;
use super::InterpCpu;

/// Single direct-mapped I-cache line (32 bytes, 512 lines = 16 KiB).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ICacheLine {
    pub valid: bool,
    pub tag: u32,
    pub data: [u8; 32],
}

/// Single direct-mapped write-back D-cache line (16 bytes, 512 lines = 8 KiB).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DCacheLine {
    pub valid: bool,
    pub dirty: bool,
    pub tag: u32,
    pub data: [u8; 16],
}

/// Functional VR4300 L1 I-cache (16 KiB) and write-back D-cache (8 KiB).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Caches {
    pub icache: Vec<ICacheLine>,
    pub dcache: Vec<DCacheLine>,
    pub icache_hits: u64,
    pub icache_misses: u64,
    pub dcache_hits: u64,
    pub dcache_misses: u64,
}

impl Caches {
    pub const ICACHE_LINES: usize = 512;
    pub const DCACHE_LINES: usize = 512;

    #[must_use]
    pub fn new() -> Self {
        Self {
            icache: vec![ICacheLine::default(); Self::ICACHE_LINES],
            dcache: vec![DCacheLine::default(); Self::DCACHE_LINES],
            icache_hits: 0,
            icache_misses: 0,
            dcache_hits: 0,
            dcache_misses: 0,
        }
    }

    pub fn reset(&mut self) {
        self.icache.fill(ICacheLine::default());
        self.dcache.fill(DCacheLine::default());
        self.icache_hits = 0;
        self.icache_misses = 0;
        self.dcache_hits = 0;
        self.dcache_misses = 0;
    }

    #[must_use]
    pub fn icache_index(pa: u32) -> usize {
        ((pa >> 5) & 0x1FF) as usize
    }

    #[must_use]
    pub fn dcache_index(pa: u32) -> usize {
        ((pa >> 4) & 0x1FF) as usize
    }

    #[must_use]
    pub fn tag_from_pa(pa: u32) -> u32 {
        pa >> 12
    }

    pub fn writeback_dcache_line(line: &mut DCacheLine, index: usize, bus: &mut Bus) {
        if line.valid && line.dirty {
            let base_pa = (line.tag << 12) | (((index as u32) << 4) & 0xFFF);
            if base_pa <= 0x03EF_FFF0 {
                for (i, &byte) in line.data.iter().enumerate() {
                    bus.rdram.write_u8(base_pa + i as u32, byte);
                }
            }
            line.dirty = false;
        }
    }

    pub fn fill_icache_line(line: &mut ICacheLine, pa: u32, bus: &Bus) {
        let base_pa = pa & !0x1F;
        for (i, byte_ref) in line.data.iter_mut().enumerate() {
            let addr = base_pa.wrapping_add(i as u32);
            *byte_ref = if addr <= 0x03EF_FFFF {
                bus.rdram.read_u8(addr)
            } else {
                0
            };
        }
        line.tag = pa >> 12;
        line.valid = true;
    }

    pub fn fill_dcache_line(line: &mut DCacheLine, pa: u32, bus: &Bus) {
        let base_pa = pa & !0x0F;
        for (i, byte_ref) in line.data.iter_mut().enumerate() {
            let addr = base_pa.wrapping_add(i as u32);
            *byte_ref = if addr <= 0x03EF_FFFF {
                bus.rdram.read_u8(addr)
            } else {
                0
            };
        }
        line.tag = pa >> 12;
        line.valid = true;
        line.dirty = false;
    }
}

impl Default for Caches {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;

/// Execute `CACHE` instruction (opcode `0x2F`) per VR4300 UM Chapter 11.
pub fn exec_cache(cpu: &mut InterpCpu, bus: &mut Bus, pc: u64, inst: Inst) -> Result<u64, Trap> {
    let va = cpu.gpr(inst.rs()).wrapping_add(inst.simm16() as u64);
    let op = inst.rt();
    let action = (op >> 2) & 7;

    // Hit ops (action >= 4), I-cache Fill (0x14), and D-cache Create Dirty Exclusive (0x0D)
    // require VA->PA translation; Index ops (0x00/0x01/0x04/0x05/0x08/0x09) do not raise TLB miss.
    let needs_tlb = action >= 4 || op == 0x14 || op == 0x0D;

    let pa = if needs_tlb {
        match mmu::translate_va_cpu(cpu, va, false) {
            Ok((p, _)) => p,
            Err(exc) => return Err(cpu.raise_exception(pc, exc)),
        }
    } else {
        va as u32
    };

    match op {
        0x00 => {
            // I-cache Index Invalidate
            let idx = Caches::icache_index(va as u32);
            cpu.caches.icache[idx].valid = false;
        }
        0x01 => {
            // D-cache Index Writeback Invalidate
            let idx = Caches::dcache_index(va as u32);
            Caches::writeback_dcache_line(&mut cpu.caches.dcache[idx], idx, bus);
            cpu.caches.dcache[idx].valid = false;
        }
        0x04 => {
            // I-cache Index Load Tag
            let idx = Caches::icache_index(va as u32);
            let line = &cpu.caches.icache[idx];
            cpu.cop0.tag_lo = (line.tag << 8) | if line.valid { 0x80 } else { 0 };
            cpu.cop0.tag_hi = 0;
        }
        0x05 => {
            // D-cache Index Load Tag
            let idx = Caches::dcache_index(va as u32);
            let line = &cpu.caches.dcache[idx];
            let state = if !line.valid {
                0
            } else if line.dirty {
                0xC0
            } else {
                0x80
            };
            cpu.cop0.tag_lo = (line.tag << 8) | state;
            cpu.cop0.tag_hi = 0;
        }
        0x08 => {
            // I-cache Index Store Tag
            let idx = Caches::icache_index(va as u32);
            let line = &mut cpu.caches.icache[idx];
            line.tag = (cpu.cop0.tag_lo >> 8) & 0x000F_FFFF;
            line.valid = (cpu.cop0.tag_lo & 0xC0) != 0;
        }
        0x09 => {
            // D-cache Index Store Tag
            let idx = Caches::dcache_index(va as u32);
            let line = &mut cpu.caches.dcache[idx];
            line.tag = (cpu.cop0.tag_lo >> 8) & 0x000F_FFFF;
            let state = (cpu.cop0.tag_lo >> 6) & 3;
            line.valid = state != 0;
            line.dirty = state == 3;
        }
        0x0D => {
            // D-cache Create Dirty Exclusive
            let idx = Caches::dcache_index(pa);
            let tag = Caches::tag_from_pa(pa);
            let line = &mut cpu.caches.dcache[idx];
            if line.valid && line.tag != tag {
                Caches::writeback_dcache_line(line, idx, bus);
            }
            line.tag = tag;
            line.valid = true;
            line.dirty = true;
        }
        0x10 => {
            // I-cache Hit Invalidate
            let idx = Caches::icache_index(pa);
            let tag = Caches::tag_from_pa(pa);
            let line = &mut cpu.caches.icache[idx];
            if line.valid && line.tag == tag {
                line.valid = false;
            }
        }
        0x11 => {
            // D-cache Hit Invalidate (discards dirty data without writeback)
            let idx = Caches::dcache_index(pa);
            let tag = Caches::tag_from_pa(pa);
            let line = &mut cpu.caches.dcache[idx];
            if line.valid && line.tag == tag {
                line.valid = false;
                line.dirty = false;
            }
        }
        0x14 => {
            // I-cache Fill
            let idx = Caches::icache_index(pa);
            Caches::fill_icache_line(&mut cpu.caches.icache[idx], pa, bus);
        }
        0x15 => {
            // D-cache Hit Writeback Invalidate
            let idx = Caches::dcache_index(pa);
            let tag = Caches::tag_from_pa(pa);
            let line = &mut cpu.caches.dcache[idx];
            if line.valid && line.tag == tag {
                Caches::writeback_dcache_line(line, idx, bus);
                line.valid = false;
            }
        }
        0x19 => {
            // D-cache Hit Writeback
            let idx = Caches::dcache_index(pa);
            let tag = Caches::tag_from_pa(pa);
            let line = &mut cpu.caches.dcache[idx];
            if line.valid && line.tag == tag {
                Caches::writeback_dcache_line(line, idx, bus);
            }
        }
        _ => {
            return Err(Trap::UnimplementedOpcode {
                pc,
                word: inst.raw(),
            })
        }
    }

    Ok(1)
}
