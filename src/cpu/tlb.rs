//! VR4300 32-entry fully associative joint TLB.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 5 (TLB)
//!   via docs/01-cpu-r4300i.md

use serde::{Deserialize, Serialize};

use crate::bus::CacheAttr;
use crate::trap::Trap;

use super::cop0::Cop0;
use super::decode::Inst;
use super::{CpuException, InterpCpu};

/// Single joint TLB entry mapping an even/odd page pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlbEntry {
    pub page_mask: u32,
    pub vpn2: u64,
    pub region: u8,
    pub asid: u8,
    pub global: bool,
    pub pfn0: u32,
    pub c0: u8,
    pub d0: bool,
    pub v0: bool,
    pub pfn1: u32,
    pub c1: u8,
    pub d1: bool,
    pub v1: bool,
}

impl Default for TlbEntry {
    fn default() -> Self {
        Self {
            page_mask: 0,
            // Initialize VPN2 to KSEG0 (0x8000_0000) so uninitialized TLB slots never match
            // TLB-mapped KUSEG/KSSEG/KSEG3 virtual addresses (returns Refill instead of Invalid).
            vpn2: 0x0000_0000_8000_0000,
            region: 0,
            asid: 0,
            global: false,
            pfn0: 0,
            c0: 0,
            d0: false,
            v0: false,
            pfn1: 0,
            c1: 0,
            d1: false,
            v1: false,
        }
    }
}

/// 32-entry fully associative VR4300 joint TLB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tlb {
    pub entries: Vec<TlbEntry>,
}

impl Tlb {
    pub const NUM_ENTRIES: usize = 32;

    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: vec![TlbEntry::default(); Self::NUM_ENTRIES],
        }
    }

    pub fn reset(&mut self) {
        self.entries.fill(TlbEntry::default());
    }

    /// Translate a TLB-mapped virtual address (`KUSEG`, `KSSEG`, `KSEG3`) into
    /// `(physical_addr, CacheAttr)` or a TLB Refill / Invalid / Modification exception.
    pub fn translate(
        &self,
        va: u64,
        is_write: bool,
        cop0: &Cop0,
    ) -> Result<(u32, CacheAttr), CpuException> {
        // Find matching TLB entry
        let mut matched_entry: Option<&TlbEntry> = None;

        for entry in &self.entries {
            let pair_mask = u64::from(entry.page_mask | 0x1FFF);
            let asid_match = entry.global || (entry.asid == cop0.asid());

            if cop0.kx() {
                let region = ((va >> 62) & 3) as u8;
                if region == entry.region {
                    let va_vpn2 = va & 0x0000_00FF_FFFF_E000;
                    let entry_vpn2 = entry.vpn2 & 0x0000_00FF_FFFF_E000;
                    if (va_vpn2 & !pair_mask) == (entry_vpn2 & !pair_mask) && asid_match {
                        matched_entry = Some(entry);
                        break;
                    }
                }
            } else {
                // 32-bit mode
                let va32 = va as u32;
                let entry_vpn2_32 = (entry.vpn2 & 0xFFFF_FFFF) as u32;
                let mask32 = (!pair_mask & 0xFFFF_FFFF) as u32;
                if (va32 & mask32) == (entry_vpn2_32 & mask32) && asid_match {
                    matched_entry = Some(entry);
                    break;
                }
            }
        }

        let Some(entry) = matched_entry else {
            // No matching entry -> TLB Refill
            return Err(if is_write {
                CpuException::TlbStore {
                    bad_vaddr: va,
                    refill: true,
                }
            } else {
                CpuException::TlbLoad {
                    bad_vaddr: va,
                    refill: true,
                }
            });
        };

        let pair_mask = u64::from(entry.page_mask | 0x1FFF);
        let page_offset_mask = pair_mask >> 1;
        let odd_bit = page_offset_mask + 1;

        let is_odd = (va & odd_bit) != 0;

        let (pfn, c, d, v) = if is_odd {
            (entry.pfn1, entry.c1, entry.d1, entry.v1)
        } else {
            (entry.pfn0, entry.c0, entry.d0, entry.v0)
        };

        if !v {
            return Err(if is_write {
                CpuException::TlbStore {
                    bad_vaddr: va,
                    refill: false,
                }
            } else {
                CpuException::TlbLoad {
                    bad_vaddr: va,
                    refill: false,
                }
            });
        }

        if is_write && !d {
            return Err(CpuException::TlbModification { bad_vaddr: va });
        }

        let pfn_shift_mask = !page_offset_mask as u32;
        let pa = ((pfn << 12) & pfn_shift_mask) | ((va as u32) & (page_offset_mask as u32));

        let cache = if c == 2 {
            CacheAttr::Uncached
        } else {
            CacheAttr::Cached
        };

        Ok((pa, cache))
    }
}

impl Default for Tlb {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute `TLBR` (`0x01`), `TLBWI` (`0x02`), `TLBWR` (`0x06`), `TLBP` (`0x08`).
pub fn exec_tlb_op(cpu: &mut InterpCpu, pc: u64, inst: Inst) -> Result<u64, Trap> {
    match inst.funct() {
        0x01 => {
            // TLBR: Read Indexed TLB Entry
            let idx = (cpu.cop0.index & 0x1F) as usize;
            let e = cpu.tlb.entries[idx];
            cpu.cop0.page_mask = e.page_mask;
            cpu.cop0.entry_hi = ((u64::from(e.region) & 3) << 62)
                | (e.vpn2 & !(u64::from(e.page_mask) | 0x1FFF) & 0x0000_00FF_FFFF_E000)
                | u64::from(e.asid);
            cpu.cop0.entry_lo0 = (u64::from(e.pfn0) << 6)
                | (u64::from(e.c0 & 7) << 3)
                | (u64::from(e.d0) << 2)
                | (u64::from(e.v0) << 1)
                | u64::from(e.global);
            cpu.cop0.entry_lo1 = (u64::from(e.pfn1) << 6)
                | (u64::from(e.c1 & 7) << 3)
                | (u64::from(e.d1) << 2)
                | (u64::from(e.v1) << 1)
                | u64::from(e.global);
        }
        0x02 | 0x06 => {
            // TLBWI / TLBWR
            let idx = if inst.funct() == 0x02 {
                (cpu.cop0.index & 0x1F) as usize
            } else {
                (cpu.cop0.random & 0x1F) as usize
            };
            let pm = cpu.cop0.page_mask & 0x01FF_E000;
            let global = ((cpu.cop0.entry_lo0 & 1) != 0) && ((cpu.cop0.entry_lo1 & 1) != 0);
            cpu.tlb.entries[idx] = TlbEntry {
                page_mask: pm,
                vpn2: cpu.cop0.entry_hi & !(u64::from(pm) | 0x1FFF) & 0x0000_00FF_FFFF_E000,
                region: ((cpu.cop0.entry_hi >> 62) & 3) as u8,
                asid: (cpu.cop0.entry_hi & 0xFF) as u8,
                global,
                pfn0: ((cpu.cop0.entry_lo0 >> 6) & 0xF_FFFF) as u32,
                c0: ((cpu.cop0.entry_lo0 >> 3) & 7) as u8,
                d0: (cpu.cop0.entry_lo0 & 4) != 0,
                v0: (cpu.cop0.entry_lo0 & 2) != 0,
                pfn1: ((cpu.cop0.entry_lo1 >> 6) & 0xF_FFFF) as u32,
                c1: ((cpu.cop0.entry_lo1 >> 3) & 7) as u8,
                d1: (cpu.cop0.entry_lo1 & 4) != 0,
                v1: (cpu.cop0.entry_lo1 & 2) != 0,
            };
        }
        0x08 => {
            // TLBP: Probe TLB for Matching Entry
            let mut found = false;
            let mut found_idx = 0_usize;
            for (i, entry) in cpu.tlb.entries.iter().enumerate() {
                let pair_mask = u64::from(entry.page_mask | 0x1FFF);
                let asid_match = entry.global || (entry.asid == cpu.cop0.asid());
                let va_vpn2 = cpu.cop0.entry_hi & 0x0000_00FF_FFFF_E000;
                let entry_vpn2 = entry.vpn2 & 0x0000_00FF_FFFF_E000;
                let region_match =
                    !cpu.cop0.kx() || (((cpu.cop0.entry_hi >> 62) & 3) as u8 == entry.region);

                if (va_vpn2 & !pair_mask) == (entry_vpn2 & !pair_mask) && asid_match && region_match
                {
                    found = true;
                    found_idx = i;
                    break;
                }
            }

            if found {
                cpu.cop0.index = found_idx as u32;
            } else {
                cpu.cop0.index = 0x8000_0000;
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

#[cfg(test)]
#[path = "tlb_tests.rs"]
mod tests;
