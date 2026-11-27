//! VR4300 virtual-to-physical MMU translation (KSEG0/KSEG1 direct + 32-entry TLB hook).
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 5 (Memory Management)
//!   via docs/01-cpu-r4300i.md

use crate::bus::CacheAttr;

use super::cop0::Cop0;
use super::exceptions::CpuException;
use super::tlb::Tlb;
use super::InterpCpu;

/// Translate a 64-bit virtual address in 32-bit kernel addressing mode into
/// `(physical_addr, CacheAttr)` using KSEG0/KSEG1 fast path and the joint TLB.
pub fn translate_va_with_tlb(
    va: u64,
    is_write: bool,
    tlb: &Tlb,
    cop0: &Cop0,
) -> Result<(u32, CacheAttr), CpuException> {
    let sign_extended = (va as i32 as i64) as u64;
    if va != sign_extended {
        // 64-bit XKPHYS direct physical segments when Status.KX == 1.
        if cop0.kx() && (va >> 62) == 0b10 {
            let cca = (va >> 59) & 0x7;
            // Bits 58..32 must be zero for valid physical memory space on VR4300.
            if (va & 0x07FF_FFFF_0000_0000) == 0 {
                let cache = if cca == 2 {
                    CacheAttr::Uncached
                } else {
                    CacheAttr::Cached
                };
                return Ok((va as u32, cache));
            }
        }
        return Err(if is_write {
            CpuException::AddressErrorStore { bad_vaddr: va }
        } else {
            CpuException::AddressErrorLoad { bad_vaddr: va }
        });
    }

    let va32 = va as u32;
    match va32 {
        // KSEG0: cached direct-mapped low 512 MiB physical window.
        0x8000_0000..=0x9FFF_FFFF => Ok((va32 & 0x1FFF_FFFF, CacheAttr::Cached)),
        // KSEG1: uncached direct-mapped low 512 MiB physical window.
        0xA000_0000..=0xBFFF_FFFF => Ok((va32 & 0x1FFF_FFFF, CacheAttr::Uncached)),
        // KUSEG / KSSEG / KSEG3: 32-entry joint TLB lookup.
        _ => tlb.translate(va, is_write, cop0),
    }
}

/// Convenience wrapper pulling TLB and COP0 from [`InterpCpu`].
pub fn translate_va_cpu(
    cpu: &InterpCpu,
    va: u64,
    is_write: bool,
) -> Result<(u32, CacheAttr), CpuException> {
    translate_va_with_tlb(va, is_write, &cpu.tlb, &cpu.cop0)
}

/// Translate an instruction-fetch virtual address, enforcing 4-byte alignment first.
pub fn translate_fetch_va(
    va: u64,
    tlb: &Tlb,
    cop0: &Cop0,
) -> Result<(u32, CacheAttr), CpuException> {
    if va & 3 != 0 {
        return Err(CpuException::AddressErrorLoad { bad_vaddr: va });
    }
    translate_va_with_tlb(va, false, tlb, cop0)
}
