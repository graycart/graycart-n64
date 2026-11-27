//! Unit tests for VR4300 integer loads, stores, unaligned, LL/SC, and SYNC family.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 16
//!   via docs/01-cpu-r4300i.md

use crate::bus::{Bus, CacheAttr};
use crate::cpu::decode::Inst;
use crate::cpu::load_store::{exec_primary, exec_sync};
use crate::cpu::InterpCpu;
use crate::trap::Trap;

fn encode_i_type(opcode: u8, rs: usize, rt: usize, imm: u16) -> Inst {
    Inst(
        ((opcode as u32) & 0x3F) << 26
            | ((rs as u32) & 0x1F) << 21
            | ((rt as u32) & 0x1F) << 16
            | (imm as u32),
    )
}

fn encode_r_type(opcode: u8, rs: usize, rt: usize, rd: usize, sa: u32, funct: u8) -> Inst {
    Inst(
        ((opcode as u32) & 0x3F) << 26
            | ((rs as u32) & 0x1F) << 21
            | ((rt as u32) & 0x1F) << 16
            | ((rd as u32) & 0x1F) << 11
            | (sa & 0x1F) << 6
            | ((funct as u32) & 0x3F),
    )
}

#[test]
fn test_load_sign_zero_extension() {
    let mut cpu = InterpCpu::new();
    let mut bus = Bus::new(false, false);

    cpu.set_gpr(1, 0xFFFF_FFFF_8000_0000);
    bus.write_u8(0, 0x80, CacheAttr::Cached).unwrap();
    bus.write_u16(2, 0x8000, CacheAttr::Cached).unwrap();
    bus.write_u32(4, 0x8000_0000, CacheAttr::Cached).unwrap();
    bus.write_u64(8, 0x8000_0000_0000_0000, CacheAttr::Cached)
        .unwrap();

    let inst = encode_i_type(0x20, 1, 2, 0);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(2), 0xFFFF_FFFF_FFFF_FF80);

    let inst = encode_i_type(0x24, 1, 3, 0);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(3), 0x0000_0000_0000_0080);

    let inst = encode_i_type(0x21, 1, 4, 2);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(4), 0xFFFF_FFFF_FFFF_8000);

    let inst = encode_i_type(0x25, 1, 5, 2);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(5), 0x0000_0000_0000_8000);

    let inst = encode_i_type(0x23, 1, 6, 4);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(6), 0xFFFF_FFFF_8000_0000);

    let inst = encode_i_type(0x27, 1, 7, 4);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(7), 0x0000_0000_8000_0000);

    let inst = encode_i_type(0x37, 1, 8, 8);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(8), 0x8000_0000_0000_0000);
}

#[test]
fn test_store_big_endian_rdram() {
    let mut cpu = InterpCpu::new();
    let mut bus = Bus::new(false, false);

    cpu.set_gpr(1, 0xFFFF_FFFF_8000_0010);
    cpu.set_gpr(2, 0x1234_5678_9ABC_DEF0);

    let inst = encode_i_type(0x28, 1, 2, 0);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(bus.read_u8(0x10, CacheAttr::Cached).unwrap(), 0xF0);

    let inst = encode_i_type(0x29, 1, 2, 2);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(bus.read_u16(0x12, CacheAttr::Cached).unwrap(), 0xDEF0);

    let inst = encode_i_type(0x2B, 1, 2, 4);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(bus.read_u32(0x14, CacheAttr::Cached).unwrap(), 0x9ABC_DEF0);

    let inst = encode_i_type(0x3F, 1, 2, 8);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(
        bus.read_u64(0x18, CacheAttr::Cached).unwrap(),
        0x1234_5678_9ABC_DEF0
    );
}

#[test]
fn test_lwl_lwr_unaligned() {
    let mut cpu = InterpCpu::new();
    let mut bus = Bus::new(false, false);

    bus.write_u32(0, 0x1234_5678, CacheAttr::Cached).unwrap();

    // off = 0
    cpu.set_gpr(1, 0xFFFF_FFFF_8000_0000);
    cpu.set_gpr(2, 0xFFFF_FFFF_AAAA_AAAA);
    let inst = encode_i_type(0x22, 1, 2, 0);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(2), 0x0000_0000_1234_5678);

    cpu.set_gpr(2, 0xFFFF_FFFF_AAAA_AAAA);
    let inst = encode_i_type(0x26, 1, 2, 0);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(2), 0xFFFF_FFFF_AAAA_AA12);

    // off = 1
    cpu.set_gpr(1, 0xFFFF_FFFF_8000_0001);
    cpu.set_gpr(2, 0xFFFF_FFFF_AAAA_AAAA);
    let inst = encode_i_type(0x22, 1, 2, 0);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(2), 0x0000_0000_3456_78AA);

    cpu.set_gpr(2, 0xFFFF_FFFF_AAAA_AAAA);
    let inst = encode_i_type(0x26, 1, 2, 0);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(2), 0xFFFF_FFFF_AAAA_1234);

    // off = 2
    cpu.set_gpr(1, 0xFFFF_FFFF_8000_0002);
    cpu.set_gpr(2, 0xFFFF_FFFF_AAAA_AAAA);
    let inst = encode_i_type(0x22, 1, 2, 0);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(2), 0x0000_0000_5678_AAAA);

    cpu.set_gpr(2, 0xFFFF_FFFF_AAAA_AAAA);
    let inst = encode_i_type(0x26, 1, 2, 0);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(2), 0xFFFF_FFFF_AA12_3456);

    // off = 3
    cpu.set_gpr(1, 0xFFFF_FFFF_8000_0003);
    cpu.set_gpr(2, 0xFFFF_FFFF_AAAA_AAAA);
    let inst = encode_i_type(0x22, 1, 2, 0);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(2), 0x0000_0000_78AA_AAAA);

    cpu.set_gpr(2, 0xFFFF_FFFF_AAAA_AAAA);
    let inst = encode_i_type(0x26, 1, 2, 0);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(2), 0x0000_0000_1234_5678);
}

#[test]
fn test_swl_swr_unaligned() {
    let mut cpu = InterpCpu::new();
    let mut bus = Bus::new(false, false);

    for off in 0..4 {
        bus.write_u32(0, 0x0000_0000, CacheAttr::Cached).unwrap();
        cpu.set_gpr(1, 0xFFFF_FFFF_8000_0000 + off);
        cpu.set_gpr(2, 0x1234_5678);

        let inst = encode_i_type(0x2A, 1, 2, 0);
        exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
        let inst = encode_i_type(0x2E, 1, 2, 0);
        exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    }
}

#[test]
fn test_ldl_ldr_sdl_sdr_roundtrip_all_offsets() {
    let mut cpu = InterpCpu::new();
    let mut bus = Bus::new(false, false);

    // Verify SDL + SDR writes an unaligned 64-bit value at every byte offset 0..=7
    // without clobbering surrounding sentinel bytes, and LDL + LDR reads it back identically.
    let val = 0x1122_3344_5566_7788u64;
    for off in 0..8u64 {
        bus.write_u64(0x100, 0xFFFF_FFFF_FFFF_FFFF, CacheAttr::Cached)
            .unwrap();
        bus.write_u64(0x108, 0xFFFF_FFFF_FFFF_FFFF, CacheAttr::Cached)
            .unwrap();
        cpu.set_gpr(1, 0xFFFF_FFFF_8000_0100 + off);
        cpu.set_gpr(2, val);

        // SDL at va + 0, SDR at va + 7
        exec_primary(&mut cpu, &mut bus, 0x1000, encode_i_type(0x2C, 1, 2, 0)).unwrap();
        exec_primary(&mut cpu, &mut bus, 0x1004, encode_i_type(0x2D, 1, 2, 7)).unwrap();

        // Read back with LDL at va + 0, LDR at va + 7 into r3
        cpu.set_gpr(3, 0xDEAD_BEEF_CAFE_BABE);
        exec_primary(&mut cpu, &mut bus, 0x1008, encode_i_type(0x1A, 1, 3, 0)).unwrap();
        exec_primary(&mut cpu, &mut bus, 0x100C, encode_i_type(0x1B, 1, 3, 7)).unwrap();
        assert_eq!(
            cpu.gpr(3),
            val,
            "64-bit unaligned roundtrip failed at off={off}"
        );
    }
}

#[test]
fn test_misaligned_traps() {
    let mut cpu = InterpCpu::new();
    let mut bus = Bus::new(false, false);

    cpu.set_gpr(1, 0xFFFF_FFFF_8000_0001);

    let inst = encode_i_type(0x21, 1, 2, 0);
    let res = exec_primary(&mut cpu, &mut bus, 0x1000, inst);
    assert!(matches!(
        res,
        Err(Trap::CpuException {
            code: "AdEL",
            bad_vaddr: Some(0xFFFF_FFFF_8000_0001),
            ..
        })
    ));
    assert_eq!(cpu.last_bad_vaddr(), 0xFFFF_FFFF_8000_0001);

    cpu.set_gpr(1, 0xFFFF_FFFF_8000_0002);
    let inst = encode_i_type(0x23, 1, 2, 0);
    let res = exec_primary(&mut cpu, &mut bus, 0x1000, inst);
    assert!(matches!(
        res,
        Err(Trap::CpuException {
            code: "AdEL",
            bad_vaddr: Some(0xFFFF_FFFF_8000_0002),
            ..
        })
    ));

    cpu.set_gpr(1, 0xFFFF_FFFF_8000_0004);
    let inst = encode_i_type(0x37, 1, 2, 0);
    let res = exec_primary(&mut cpu, &mut bus, 0x1000, inst);
    assert!(matches!(
        res,
        Err(Trap::CpuException {
            code: "AdEL",
            bad_vaddr: Some(0xFFFF_FFFF_8000_0004),
            ..
        })
    ));

    cpu.set_gpr(1, 0xFFFF_FFFF_8000_0001);
    let inst = encode_i_type(0x29, 1, 2, 0);
    let res = exec_primary(&mut cpu, &mut bus, 0x1000, inst);
    assert!(matches!(
        res,
        Err(Trap::CpuException {
            code: "AdES",
            bad_vaddr: Some(0xFFFF_FFFF_8000_0001),
            ..
        })
    ));

    cpu.set_gpr(1, 0xFFFF_FFFF_8000_0002);
    let inst = encode_i_type(0x2B, 1, 2, 0);
    let res = exec_primary(&mut cpu, &mut bus, 0x1000, inst);
    assert!(matches!(
        res,
        Err(Trap::CpuException {
            code: "AdES",
            bad_vaddr: Some(0xFFFF_FFFF_8000_0002),
            ..
        })
    ));
}

#[test]
fn test_ll_sc_semantics() {
    let mut cpu = InterpCpu::new();
    let mut bus = Bus::new(false, false);

    cpu.set_gpr(1, 0xFFFF_FFFF_8000_0000);
    cpu.set_gpr(2, 0xDEAD_BEEF);

    let inst = encode_i_type(0x30, 1, 2, 0);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert!(cpu.ll_bit());

    cpu.set_gpr(2, 0x1234_5678);
    let inst = encode_i_type(0x38, 1, 2, 0);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(2), 1);
    assert!(!cpu.ll_bit());
    assert_eq!(bus.read_u32(0, CacheAttr::Cached).unwrap(), 0x1234_5678);

    cpu.set_gpr(2, 0x8765_4321);
    let inst = encode_i_type(0x38, 1, 2, 0);
    exec_primary(&mut cpu, &mut bus, 0x1000, inst).unwrap();
    assert_eq!(cpu.gpr(2), 0);
    assert_eq!(bus.read_u32(0, CacheAttr::Cached).unwrap(), 0x1234_5678);
}

#[test]
fn test_cached_mmio_freeze() {
    let mut cpu = InterpCpu::new();
    let mut bus = Bus::new(false, false);

    cpu.set_gpr(1, 0xFFFF_FFFF_8430_0000);
    let inst = encode_i_type(0x23, 1, 2, 0);
    let res = exec_primary(&mut cpu, &mut bus, 0x1000, inst);
    assert!(matches!(
        res,
        Err(Trap::CachedMmioFreeze { pa: 0x0430_0000 })
    ));
}

#[test]
fn test_sync_and_unknown() {
    let mut cpu = InterpCpu::new();
    let inst = encode_r_type(0x00, 0, 0, 0, 0, 0x0F);
    assert_eq!(exec_sync(&mut cpu, 0x1000, inst).unwrap(), 1);

    let bad_inst = encode_r_type(0x00, 0, 0, 0, 0, 0x00);
    assert!(matches!(
        exec_sync(&mut cpu, 0x1000, bad_inst),
        Err(Trap::UnimplementedOpcode { .. })
    ));
}
