//! Unit tests for physical decode map, SysAD freeze traps, RDRAM 9th-bit array,
//! and ISViewer debug console capture.

use super::*;

#[test]
fn physical_map_classifies_all_canonical_regions() {
    assert_eq!(decode_physical(0x0000_0000, true), PhysRegion::Rdram);
    assert_eq!(decode_physical(0x03EF_FFFF, true), PhysRegion::Rdram);
    assert_eq!(decode_physical(0x03F0_0000, true), PhysRegion::RdramRegs);
    assert_eq!(decode_physical(0x0400_0000, true), PhysRegion::RspDmem);
    assert_eq!(decode_physical(0x0400_0FFF, true), PhysRegion::RspDmem);
    assert_eq!(decode_physical(0x0400_1000, true), PhysRegion::RspImem);
    assert_eq!(decode_physical(0x0400_1FFF, true), PhysRegion::RspImem);
    // 8 KiB DMEM/IMEM mirror window
    assert_eq!(decode_physical(0x0400_2000, true), PhysRegion::RspDmem);
    assert_eq!(decode_physical(0x0400_3000, true), PhysRegion::RspImem);
    assert_eq!(decode_physical(0x0404_0000, true), PhysRegion::RspRegs);
    assert_eq!(
        decode_physical(0x040C_0000, true),
        PhysRegion::UnmappedFreeze
    );
    assert_eq!(
        decode_physical(0x040F_FFFF, true),
        PhysRegion::UnmappedFreeze
    );
    assert_eq!(decode_physical(0x0410_0000, true), PhysRegion::RdpCmdRegs);
    assert_eq!(decode_physical(0x0420_0000, true), PhysRegion::RdpSpanRegs);
    assert_eq!(decode_physical(0x0430_0000, true), PhysRegion::MiRegs);
    assert_eq!(decode_physical(0x0440_0000, true), PhysRegion::ViRegs);
    assert_eq!(decode_physical(0x0450_0000, true), PhysRegion::AiRegs);
    assert_eq!(decode_physical(0x0460_0000, true), PhysRegion::PiRegs);
    assert_eq!(decode_physical(0x0470_0000, true), PhysRegion::RiRegs);
    assert_eq!(decode_physical(0x0480_0000, true), PhysRegion::SiRegs);
    assert_eq!(
        decode_physical(0x0490_0000, true),
        PhysRegion::UnmappedFreeze
    );
    assert_eq!(
        decode_physical(0x04FF_FFFF, true),
        PhysRegion::UnmappedFreeze
    );
    assert_eq!(decode_physical(0x0500_0000, true), PhysRegion::DdRegs);
    assert_eq!(decode_physical(0x0600_0000, true), PhysRegion::DdIplRom);
    assert_eq!(
        decode_physical(0x0800_0000, true),
        PhysRegion::CartSramFlash
    );
    assert_eq!(decode_physical(0x1000_0000, true), PhysRegion::CartRom);
    assert_eq!(decode_physical(0x13FF_0000, true), PhysRegion::IsViewer);
    assert_eq!(decode_physical(0x13FF_0000, false), PhysRegion::CartRom);
    assert_eq!(decode_physical(0x1FC0_0000, true), PhysRegion::PifRom);
    assert_eq!(decode_physical(0x1FC0_07C0, true), PhysRegion::PifRam);
    assert_eq!(decode_physical(0x1FC0_07FF, true), PhysRegion::PifRam);
    assert_eq!(
        decode_physical(0x1FC0_0800, true),
        PhysRegion::UnmappedFreeze
    );
    assert_eq!(
        decode_physical(0x8000_0000, true),
        PhysRegion::UnmappedFreeze
    );
    assert_eq!(
        decode_physical(0xFFFF_FFFF, true),
        PhysRegion::UnmappedFreeze
    );
}

#[test]
fn cached_access_outside_rdram_triggers_hard_freeze_trap() {
    let mut bus = Bus::new(true, true);
    // RDRAM cached access is legal.
    bus.write_u32(0x0000_0100, 0xDEAD_BEEF, CacheAttr::Cached)
        .expect("cached RDRAM write must succeed");
    assert_eq!(
        bus.read_u32(0x0000_0100, CacheAttr::Cached),
        Ok(0xDEAD_BEEF)
    );

    // Cached MMIO / RCP / PIF / Cart accesses MUST trap.
    for pa in [
        0x0400_0000u32,
        0x0430_0000,
        0x0460_0000,
        0x1000_0000,
        0x1FC0_07C0,
    ] {
        assert_eq!(
            bus.read_u32(pa, CacheAttr::Cached),
            Err(Trap::CachedMmioFreeze { pa }),
            "expected cached freeze at PA 0x{pa:08X}"
        );
    }
}

#[test]
fn unmapped_windows_and_64bit_rcp_reads_trap() {
    let bus = Bus::new(true, true);
    assert_eq!(
        bus.read_u32(0x040C_0000, CacheAttr::Uncached),
        Err(Trap::UnmappedFreeze {
            pa: 0x040C_0000,
            size: 4,
            is_write: false,
        })
    );
    assert_eq!(
        bus.read_u32(0x0490_0000, CacheAttr::Uncached),
        Err(Trap::UnmappedFreeze {
            pa: 0x0490_0000,
            size: 4,
            is_write: false,
        })
    );
    assert_eq!(
        bus.read_u32(0x8000_0000, CacheAttr::Uncached),
        Err(Trap::UnmappedFreeze {
            pa: 0x8000_0000,
            size: 4,
            is_write: false,
        })
    );
    assert_eq!(
        bus.read_u64(0x0430_0000, CacheAttr::Uncached),
        Err(Trap::Rcp64BitReadFreeze { pa: 0x0430_0000 })
    );
}

#[test]
fn unimplemented_mmio_registers_trap_instead_of_returning_zero() {
    let mut bus = Bus::new(true, true);
    assert_eq!(
        bus.read_u32(0x0430_00FC, CacheAttr::Uncached),
        Err(Trap::UnimplementedRegister {
            device: "MI",
            offset: 0x00FC,
            is_write: false,
        })
    );
    assert_eq!(
        bus.write_u32(0x0460_00FC, 1, CacheAttr::Uncached),
        Err(Trap::UnimplementedRegister {
            device: "PI",
            offset: 0x00FC,
            is_write: true,
        })
    );
    assert_eq!(
        bus.read_u32(0x0404_0000, CacheAttr::Uncached),
        Err(Trap::UnimplementedRegister {
            device: "SP",
            offset: 0x0000,
            is_write: false,
        })
    );
}

#[test]
fn rdram_sizes_open_bank_reads_and_9th_bit_side_array() {
    let mut base = Rdram::new(false);
    assert_eq!(base.len(), 4 * 1024 * 1024);
    assert_eq!(base.hidden_bits_len(), 4 * 1024 * 1024 / 8);

    // Reads above installed 4 MiB return 0 (open bank; systemtest IPL3 >8 MiB contract).
    base.write_u32(0x0040_0000, 0x1234_5678).unwrap();
    assert_eq!(base.read_u32(0x0040_0000).unwrap(), 0);
    assert!(!base.read_hidden_bit(0x0040_0000));

    let mut exp = Rdram::new(true);
    assert_eq!(exp.len(), 8 * 1024 * 1024);
    assert_eq!(exp.hidden_bits_len(), 8 * 1024 * 1024 / 8);
    assert_eq!(exp.read_u32(0x0080_0000).unwrap(), 0);

    // 9th-bit side array isolates adjacent bytes across byte boundaries.
    for addr in [0usize, 1, 7, 8, 15, 16, 0x7F_FFFF] {
        assert!(!exp.read_hidden_bit(addr));
    }
    exp.write_hidden_bit(0, true);
    exp.write_hidden_bit(7, true);
    exp.write_hidden_bit(8, true);
    exp.write_hidden_bit(0x7F_FFFF, true);
    assert!(exp.read_hidden_bit(0));
    assert!(!exp.read_hidden_bit(1));
    assert!(!exp.read_hidden_bit(6));
    assert!(exp.read_hidden_bit(7));
    assert!(exp.read_hidden_bit(8));
    assert!(!exp.read_hidden_bit(9));
    assert!(exp.read_hidden_bit(0x7F_FFFF));

    exp.write_hidden_bit(7, false);
    assert!(!exp.read_hidden_bit(7));
    assert!(exp.read_hidden_bit(0));
    assert!(exp.read_hidden_bit(8));
}

#[test]
fn isviewer_captures_logged_bytes_and_wraps_ring_buffer() {
    let mut bus = Bus::new(true, true);
    assert_eq!(
        bus.read_u32(ISVIEWER_BASE, CacheAttr::Uncached),
        Ok(ISVIEWER_MAGIC)
    );

    let msg = b"Done! Tests: 42. Failed: 0\n";
    for (i, &b) in msg.iter().enumerate() {
        bus.write_u8(ISVIEWER_BASE + 0x20 + i as u32, b, CacheAttr::Uncached)
            .unwrap();
    }
    bus.write_u32(ISVIEWER_BASE + 0x14, msg.len() as u32, CacheAttr::Uncached)
        .unwrap();
    assert_eq!(bus.isviewer.output_lossy(), "Done! Tests: 42. Failed: 0\n");

    // Verify ring-buffer wrap around ISVIEWER_BUF_SIZE (0xFFE0).
    let start = (isviewer::ISVIEWER_BUF_SIZE - 2) as u32;
    bus.write_u32(ISVIEWER_BASE + 0x04, start, CacheAttr::Uncached)
        .unwrap();
    bus.write_u8(ISVIEWER_BASE + 0x20 + start, b'O', CacheAttr::Uncached)
        .unwrap();
    bus.write_u8(ISVIEWER_BASE + 0x20 + start + 1, b'K', CacheAttr::Uncached)
        .unwrap();
    bus.write_u8(ISVIEWER_BASE + 0x20, b'!', CacheAttr::Uncached)
        .unwrap();
    bus.write_u32(ISVIEWER_BASE + 0x14, 1, CacheAttr::Uncached)
        .unwrap();
    assert!(bus.isviewer.output_lossy().ends_with("OK!"));
}
