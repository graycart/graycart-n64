//! Unit tests for VR4300 L1 I/D caches and CACHE instruction.
//!
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual (U10504EJ7V0UMJ1), Chapter 11

use crate::bus::Bus;
use crate::cpu::cache::{exec_cache, Caches};
use crate::cpu::decode::Inst;
use crate::cpu::InterpCpu;
use crate::trap::Trap;

fn helper_make_cache_inst(rt: u8, rs: u8, simm16: i16) -> Inst {
    let raw = (0x2F << 26)
        | ((rs as u32 & 0x1F) << 21)
        | ((rt as u32 & 0x1F) << 16)
        | (simm16 as u16 as u32);
    Inst(raw)
}

#[test]
fn test_1_icache_fill_and_index_invalidate() {
    let mut bus = Bus::new(true, false);
    let mut cpu = InterpCpu::new();

    for i in 0..32 {
        bus.rdram.write_u8(0x1000 + i, (i + 0x50) as u8);
    }

    let pa = 0x1000u32;
    let idx = Caches::icache_index(pa);

    Caches::fill_icache_line(&mut cpu.caches.icache[idx], pa, &bus);
    assert!(cpu.caches.icache[idx].valid);
    assert_eq!(cpu.caches.icache[idx].tag, pa >> 12);
    assert_eq!(cpu.caches.icache[idx].data[0], 0x50);
    assert_eq!(cpu.caches.icache[idx].data[31], (31 + 0x50) as u8);

    cpu.set_gpr(1, 0x8000_1000);
    let inst = helper_make_cache_inst(0x00, 1, 0);
    let res = exec_cache(&mut cpu, &mut bus, 0x8000_0000, inst);
    assert!(res.is_ok());
    assert!(!cpu.caches.icache[idx].valid);
}

#[test]
fn test_2_tag_store_load_roundtrip() {
    let mut bus = Bus::new(true, false);
    let mut cpu = InterpCpu::new();

    cpu.cop0.tag_lo = (0x123u32 << 8) | 0x80;
    cpu.set_gpr(1, 0x8000_0040);
    let inst_store_i = helper_make_cache_inst(0x08, 1, 0);
    exec_cache(&mut cpu, &mut bus, 0, inst_store_i).unwrap();

    let inst_load_i = helper_make_cache_inst(0x04, 1, 0);
    exec_cache(&mut cpu, &mut bus, 0, inst_load_i).unwrap();
    assert_eq!(cpu.cop0.tag_lo, (0x123u32 << 8) | 0x80);

    cpu.cop0.tag_lo = (0x456u32 << 8) | 0xC0;
    let inst_store_d = helper_make_cache_inst(0x09, 1, 0);
    exec_cache(&mut cpu, &mut bus, 0, inst_store_d).unwrap();

    let inst_load_d = helper_make_cache_inst(0x05, 1, 0);
    exec_cache(&mut cpu, &mut bus, 0, inst_load_d).unwrap();
    assert_eq!(cpu.cop0.tag_lo, (0x456u32 << 8) | 0xC0);
}

#[test]
fn test_3_dcache_index_writeback_invalidate() {
    let mut bus = Bus::new(true, false);
    let mut cpu = InterpCpu::new();

    let pa = 0x2000u32;
    let idx = Caches::dcache_index(pa);
    {
        let line = &mut cpu.caches.dcache[idx];
        line.valid = true;
        line.dirty = true;
        line.tag = pa >> 12;
        line.data[0] = 0xAB;
        line.data[15] = 0xCD;
    }

    cpu.set_gpr(1, 0x8000_2000);
    let inst = helper_make_cache_inst(0x01, 1, 0);
    exec_cache(&mut cpu, &mut bus, 0, inst).unwrap();

    assert!(!cpu.caches.dcache[idx].valid);
    assert!(!cpu.caches.dcache[idx].dirty);
    assert_eq!(bus.rdram.read_u8(0x2000), 0xAB);
    assert_eq!(bus.rdram.read_u8(0x200F), 0xCD);
}

#[test]
fn test_4_dcache_hit_invalidate_discards_dirty() {
    let mut bus = Bus::new(true, false);
    let mut cpu = InterpCpu::new();

    let pa = 0x3000u32;
    let idx = Caches::dcache_index(pa);
    {
        let line = &mut cpu.caches.dcache[idx];
        line.valid = true;
        line.dirty = true;
        line.tag = pa >> 12;
        line.data[0] = 0xEF;
    }

    bus.rdram.write_u8(0x3000, 0x00);

    cpu.set_gpr(1, 0xFFFF_FFFF_8000_3000);
    let inst = helper_make_cache_inst(0x11, 1, 0);
    exec_cache(&mut cpu, &mut bus, 0, inst).unwrap();

    assert!(!cpu.caches.dcache[idx].valid);
    assert!(!cpu.caches.dcache[idx].dirty);
    assert_eq!(bus.rdram.read_u8(0x3000), 0x00);

    {
        let line = &mut cpu.caches.dcache[idx];
        line.valid = true;
        line.dirty = true;
        line.tag = 0x999;
    }
    exec_cache(&mut cpu, &mut bus, 0, inst).unwrap();
    assert!(cpu.caches.dcache[idx].valid);
}

#[test]
fn test_5_dcache_hit_writeback_invalidate() {
    let mut bus = Bus::new(true, false);
    let mut cpu = InterpCpu::new();

    let pa = 0x4000u32;
    let idx = Caches::dcache_index(pa);
    {
        let line = &mut cpu.caches.dcache[idx];
        line.valid = true;
        line.dirty = true;
        line.tag = pa >> 12;
        line.data[0] = 0x77;
    }

    cpu.set_gpr(1, 0xFFFF_FFFF_8000_4000);
    let inst = helper_make_cache_inst(0x15, 1, 0);
    exec_cache(&mut cpu, &mut bus, 0, inst).unwrap();

    assert!(!cpu.caches.dcache[idx].valid);
    assert_eq!(bus.rdram.read_u8(0x4000), 0x77);

    {
        let line = &mut cpu.caches.dcache[idx];
        line.valid = true;
        line.dirty = true;
        line.tag = 0x888;
    }
    exec_cache(&mut cpu, &mut bus, 0, inst).unwrap();
    assert!(cpu.caches.dcache[idx].valid);
}

#[test]
fn test_6_dcache_hit_writeback() {
    let mut bus = Bus::new(true, false);
    let mut cpu = InterpCpu::new();

    let pa = 0x5000u32;
    let idx = Caches::dcache_index(pa);
    {
        let line = &mut cpu.caches.dcache[idx];
        line.valid = true;
        line.dirty = true;
        line.tag = pa >> 12;
        line.data[0] = 0x55;
    }

    cpu.set_gpr(1, 0xFFFF_FFFF_8000_5000);
    let inst = helper_make_cache_inst(0x19, 1, 0);
    exec_cache(&mut cpu, &mut bus, 0, inst).unwrap();

    assert!(cpu.caches.dcache[idx].valid);
    assert!(!cpu.caches.dcache[idx].dirty);
    assert_eq!(bus.rdram.read_u8(0x5000), 0x55);
}

#[test]
fn test_7_dcache_create_dirty_exclusive() {
    let mut bus = Bus::new(true, false);
    let mut cpu = InterpCpu::new();

    let pa1 = 0x6000u32;
    // 8 KiB D-cache stride = 0x2000, so 0x6000 and 0x8000 alias to the exact same D-cache index.
    let pa2 = 0x8000u32;
    let idx = Caches::dcache_index(pa1);
    assert_eq!(idx, Caches::dcache_index(pa2));

    {
        let line = &mut cpu.caches.dcache[idx];
        line.valid = true;
        line.dirty = true;
        line.tag = pa1 >> 12;
        line.data[0] = 0x33;
    }

    cpu.set_gpr(1, 0xFFFF_FFFF_8000_8000);
    let inst = helper_make_cache_inst(0x0D, 1, 0);
    exec_cache(&mut cpu, &mut bus, 0, inst).unwrap();

    assert_eq!(bus.rdram.read_u8(pa1), 0x33);
    assert!(cpu.caches.dcache[idx].valid);
    assert!(cpu.caches.dcache[idx].dirty);
    assert_eq!(cpu.caches.dcache[idx].tag, pa2 >> 12);
}

#[test]
fn test_8_hit_vs_index_tlb_behavior() {
    let mut bus = Bus::new(true, false);
    let mut cpu = InterpCpu::new();

    cpu.set_gpr(1, 0x0000_1000);
    let hit_inst = helper_make_cache_inst(0x10, 1, 0);
    let res_hit = exec_cache(&mut cpu, &mut bus, 0xFFFF_FFFF_8000_0000, hit_inst);
    assert!(matches!(
        res_hit,
        Err(Trap::CpuException { code: "TLBL", .. })
    ));

    let index_inst = helper_make_cache_inst(0x00, 1, 0);
    let res_index = exec_cache(&mut cpu, &mut bus, 0xFFFF_FFFF_8000_0000, index_inst);
    assert!(res_index.is_ok());
}

#[test]
fn test_9_unimplemented_cache_op_traps() {
    let mut bus = Bus::new(true, false);
    let mut cpu = InterpCpu::new();

    cpu.set_gpr(1, 0xFFFF_FFFF_8000_1000);
    let bad_inst = helper_make_cache_inst(0x1F, 1, 0);
    let res = exec_cache(&mut cpu, &mut bus, 0xFFFF_FFFF_8000_0000, bad_inst);
    assert!(matches!(res, Err(Trap::UnimplementedOpcode { .. })));
}
