//! VR4300 virtual-to-physical MMU translation (M1.0 KSEG0/KSEG1 fast path; Wave B wires TLB).
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 5 (Memory Management)
//!   via docs/01-cpu-r4300i.md

use crate::bus::CacheAttr;

use super::exceptions::CpuException;

/// Translate a 64-bit virtual address in 32-bit kernel addressing mode into
/// `(physical_addr, CacheAttr)`.
///
/// - `KSEG0` (`0xFFFF_FFFF_8000_0000..=0xFFFF_FFFF_9FFF_FFFF`) -> direct `PA = VA & 0x1FFF_FFFF`, [`CacheAttr::Cached`]
/// - `KSEG1` (`0xFFFF_FFFF_A000_0000..=0xFFFF_FFFF_BFFF_FFFF`) -> direct `PA = VA & 0x1FFF_FFFF`, [`CacheAttr::Uncached`]
/// - `KUSEG` / `KSSEG` / `KSEG3` -> TLB miss (Wave B wires full 32-entry joint TLB lookup).
/// - Non-sign-extended 32-bit addresses in 32-bit mode raise `AdEL` / `AdES`.
pub fn translate_va(va: u64, is_write: bool) -> Result<(u32, CacheAttr), CpuException> {
    let sign_extended = (va as i32 as i64) as u64;
    if va != sign_extended {
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
        // KUSEG / KSSEG / KSEG3: TLB-mapped segments (Wave B implements TLB lookup).
        _ => Err(if is_write {
            CpuException::TlbStore { bad_vaddr: va }
        } else {
            CpuException::TlbLoad { bad_vaddr: va }
        }),
    }
}

/// Translate an instruction-fetch virtual address, enforcing 4-byte alignment first.
pub fn translate_fetch_va(va: u64) -> Result<(u32, CacheAttr), CpuException> {
    if va & 3 != 0 {
        return Err(CpuException::AddressErrorLoad { bad_vaddr: va });
    }
    translate_va(va, false)
}
