//! Unit tests for VR4300 32-entry fully associative joint TLB (`src/cpu/tlb.rs`).
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 5 (TLB)
//!   via docs/01-cpu-r4300i.md

#![allow(clippy::identity_op)]

use crate::bus::CacheAttr;
use crate::cpu::decode::Inst;
use crate::cpu::tlb::exec_tlb_op;
use crate::cpu::{CpuException, InterpCpu};
use crate::trap::Trap;

fn setup_cpu() -> InterpCpu {
    InterpCpu::new()
}

#[test]
fn test_tlb_4kb_even_odd_translation() {
    let mut cpu = setup_cpu();
    cpu.cop0.entry_hi = 0x0000_2000; // VPN2 = 0x2000 (covers 0x2000 even & 0x3000 odd), ASID = 0
    cpu.cop0.page_mask = 0; // 4 KiB pages
    cpu.cop0.entry_lo0 = (0x100 << 6) | (3 << 3) | (1 << 2) | (1 << 1) | 1; // PFN 0x100, cached (3), D=1, V=1, G=1
    cpu.cop0.entry_lo1 = (0x200 << 6) | (2 << 3) | (1 << 2) | (1 << 1) | 1; // PFN 0x200, uncached (2), D=1, V=1, G=1
    cpu.cop0.index = 0;

    let res = exec_tlb_op(&mut cpu, 0, Inst(0x4200_0002)); // TLBWI
    assert!(res.is_ok());

    // Even page VA = 0x2000 (bit 12 = 0)
    let (pa0, cache0) = cpu.tlb.translate(0x2000, false, &cpu.cop0).unwrap();
    assert_eq!(pa0, 0x0010_0000);
    assert_eq!(cache0, CacheAttr::Cached);

    // Odd page VA = 0x3000 (bit 12 = 1)
    let (pa1, cache1) = cpu.tlb.translate(0x3000, false, &cpu.cop0).unwrap();
    assert_eq!(pa1, 0x0020_0000);
    assert_eq!(cache1, CacheAttr::Uncached);
}

#[test]
fn test_tlb_variable_page_masks() {
    let mut cpu = setup_cpu();

    // Test 16 KiB page mask (0x0000_6000): pair covers 32 KiB (0x8000..0xFFFF)
    cpu.cop0.entry_hi = 0x0000_8000;
    cpu.cop0.page_mask = 0x0000_6000;
    cpu.cop0.entry_lo0 = (0x300 << 6) | (3 << 3) | (1 << 2) | (1 << 1) | 1;
    cpu.cop0.entry_lo1 = (0x400 << 6) | (3 << 3) | (1 << 2) | (1 << 1) | 1;
    cpu.cop0.index = 1;
    let _ = exec_tlb_op(&mut cpu, 0, Inst(0x4200_0002));

    // Even page VA = 0x8000 + 0x123
    let (pa0, _) = cpu.tlb.translate(0x8123, false, &cpu.cop0).unwrap();
    assert_eq!(pa0, 0x0030_0123);

    // Odd page VA = 0xC000 + 0x456 (bit 14 set)
    let (pa1, _) = cpu.tlb.translate(0xC456, false, &cpu.cop0).unwrap();
    assert_eq!(pa1, 0x0040_0456);
}

#[test]
fn test_tlb_asid_vs_global() {
    let mut cpu = setup_cpu();

    // Entry with G=0 (lo0.G=1, lo1.G=0 -> global = false), ASID = 0x55
    cpu.cop0.entry_hi = 0x0000_2000 | 0x55;
    cpu.cop0.page_mask = 0;
    cpu.cop0.entry_lo0 = (0x100 << 6) | (3 << 3) | (1 << 2) | (1 << 1) | 1;
    cpu.cop0.entry_lo1 = (0x200 << 6) | (3 << 3) | (1 << 2) | (1 << 1) | 0; // G=0
    cpu.cop0.index = 2;
    let _ = exec_tlb_op(&mut cpu, 0, Inst(0x42000002));

    // Match with ASID = 0x55 should succeed
    cpu.cop0.entry_hi = 0x55;
    assert!(cpu.tlb.translate(0x2000, false, &cpu.cop0).is_ok());

    // Match with ASID = 0xAA should fail (Refill)
    cpu.cop0.entry_hi = 0xAA;
    let err = cpu.tlb.translate(0x2000, false, &cpu.cop0).unwrap_err();
    assert!(err.is_tlb_refill());
}

#[test]
fn test_tlb_refill_vs_invalid() {
    let mut cpu = setup_cpu();

    // No matching entry -> Refill
    let err = cpu.tlb.translate(0x3000, false, &cpu.cop0).unwrap_err();
    assert!(err.is_tlb_refill());

    // Matching entry with V=0 -> Invalid (refill: false)
    cpu.cop0.entry_hi = 0x0000_3000;
    cpu.cop0.page_mask = 0;
    cpu.cop0.entry_lo0 = (0x100 << 6) | (3 << 3) | (1 << 2) | (0 << 1) | 1; // V=0
    cpu.cop0.entry_lo1 = (0x200 << 6) | (3 << 3) | (1 << 2) | (0 << 1) | 1; // V=0
    cpu.cop0.index = 3;
    let _ = exec_tlb_op(&mut cpu, 0, Inst(0x42000002));

    let err2 = cpu.tlb.translate(0x3000, false, &cpu.cop0).unwrap_err();
    match err2 {
        CpuException::TlbLoad { refill, .. } => assert!(!refill),
        _ => panic!("Expected TlbLoad with refill: false"),
    }
}

#[test]
fn test_tlb_modification() {
    let mut cpu = setup_cpu();

    cpu.cop0.entry_hi = 0x0000_4000;
    cpu.cop0.page_mask = 0;
    cpu.cop0.entry_lo0 = (0x100 << 6) | (3 << 3) | (0 << 2) | (1 << 1) | 1; // D=0, V=1
    cpu.cop0.entry_lo1 = (0x200 << 6) | (3 << 3) | (0 << 2) | (1 << 1) | 1; // D=0, V=1
    cpu.cop0.index = 4;
    let _ = exec_tlb_op(&mut cpu, 0, Inst(0x42000002));

    // Load (read) should succeed even with D=0
    assert!(cpu.tlb.translate(0x4000, false, &cpu.cop0).is_ok());

    // Store (write) with D=0 should trigger TlbModification
    let err = cpu.tlb.translate(0x4000, true, &cpu.cop0).unwrap_err();
    match err {
        CpuException::TlbModification { .. } => {}
        _ => panic!("Expected TlbModification"),
    }
}

#[test]
fn test_tlbwi_tlbr_roundtrip() {
    let mut cpu = setup_cpu();

    cpu.cop0.page_mask = 0x0001_E000; // 64 KiB
    cpu.cop0.entry_hi = 0x0000_5000 | 0x12; // VPN2 = 0x5000, ASID = 0x12
    cpu.cop0.entry_lo0 = (0x1234 << 6) | (5 << 3) | (1 << 2) | (1 << 1) | 1;
    cpu.cop0.entry_lo1 = (0x5678 << 6) | (4 << 3) | (0 << 2) | (1 << 1) | 1;
    cpu.cop0.index = 5;

    exec_tlb_op(&mut cpu, 0, Inst(0x42000002)).unwrap(); // TLBWI

    // Clear COP0 entry registers
    cpu.cop0.page_mask = 0;
    cpu.cop0.entry_hi = 0;
    cpu.cop0.entry_lo0 = 0;
    cpu.cop0.entry_lo1 = 0;

    exec_tlb_op(&mut cpu, 0, Inst(0x42000001)).unwrap(); // TLBR

    assert_eq!(cpu.cop0.page_mask, 0x0001_E000);
    assert_eq!(cpu.cop0.asid(), 0x12);
    assert_eq!(
        cpu.cop0.entry_lo0 & 0x3FFF_FFFF,
        (0x1234 << 6) | (5 << 3) | (1 << 2) | (1 << 1) | 1
    );
}

#[test]
fn test_tlbwr_writes_random() {
    let mut cpu = setup_cpu();
    cpu.cop0.random = 17;
    cpu.cop0.page_mask = 0;
    cpu.cop0.entry_hi = 0x0000_7000;
    cpu.cop0.entry_lo0 = (0x100 << 6) | (3 << 3) | (1 << 2) | (1 << 1) | 1;
    cpu.cop0.entry_lo1 = (0x200 << 6) | (3 << 3) | (1 << 2) | (1 << 1) | 1;

    exec_tlb_op(&mut cpu, 0, Inst(0x42000006)).unwrap(); // TLBWR

    // Check entry 17 was written
    assert_eq!(cpu.tlb.entries[17].vpn2 & 0x1FFF, 0);
}

#[test]
fn test_tlbp_probe() {
    let mut cpu = setup_cpu();

    cpu.cop0.entry_hi = 0x0000_8000 | 0x99;
    cpu.cop0.page_mask = 0;
    cpu.cop0.entry_lo0 = (0x100 << 6) | (3 << 3) | (1 << 2) | (1 << 1) | 1;
    cpu.cop0.entry_lo1 = (0x200 << 6) | (3 << 3) | (1 << 2) | (1 << 1) | 1;
    cpu.cop0.index = 10;
    exec_tlb_op(&mut cpu, 0, Inst(0x42000002)).unwrap(); // TLBWI at index 10

    // Probe matching entry with same ASID
    cpu.cop0.entry_hi = 0x0000_8000 | 0x99;
    exec_tlb_op(&mut cpu, 0, Inst(0x42000008)).unwrap(); // TLBP
    assert_eq!(cpu.cop0.index, 10);

    // Probe missing entry (0xA000 is the next 8 KiB VPN2 pair after 0x8000/0x9000)
    cpu.cop0.entry_hi = 0x0000_A000 | 0x99;
    exec_tlb_op(&mut cpu, 0, Inst(0x42000008)).unwrap(); // TLBP
    assert_eq!(cpu.cop0.index, 0x8000_0000);
}

#[test]
fn test_unknown_tlb_funct_trap() {
    let mut cpu = setup_cpu();
    let res = exec_tlb_op(&mut cpu, 0xbfc00000, Inst(0x4200003F));
    match res {
        Err(Trap::UnimplementedOpcode { pc, word }) => {
            assert_eq!(pc, 0xbfc00000);
            assert_eq!(word, 0x4200003F);
        }
        _ => panic!("Expected UnimplementedOpcode trap"),
    }
}
