//! Unit tests for RSP (SP) MMIO registers and 2D DMA engine.
//!
//! Cited: N64brew — Reality Signal Processor (Interface & DMA)
//!   https://n64brew.dev/wiki/Reality_Signal_Processor/Interface · docs/03-rsp.md §3

use super::*;
use crate::bus::Rdram;

#[test]
fn test_power_on_defaults() {
    let sp = Sp::new();
    assert_eq!(sp.status & Sp::STATUS_HALT, Sp::STATUS_HALT);
    assert_eq!(sp.pc, 0);
    assert!(!sp.semaphore.get());
    assert_eq!(sp.read_reg(0x0001C).unwrap(), 0); // initial semaphore is 0
}

#[test]
fn test_status_paired_clear_set() {
    let mut sp = Sp::new();
    // Default: HALT is set
    assert!(sp.is_halted());

    // Clear HALT (bit 0 = 1, bit 1 = 0)
    sp.write_reg_action(0x00010, 1).unwrap();
    assert!(!sp.is_halted());

    // Set HALT (bit 0 = 0, bit 1 = 1)
    sp.write_reg_action(0x00010, 2).unwrap();
    assert!(sp.is_halted());

    // Both clear and set bits set for HALT (bits 0 and 1 set) -> no-op
    sp.write_reg_action(0x00010, 1).unwrap(); // clear halt first
    assert!(!sp.is_halted());
    sp.write_reg_action(0x00010, 3).unwrap(); // both bits 0 and 1 set
    assert!(!sp.is_halted()); // should remain cleared

    // CLEAR_BROKE, CLEAR_INTR / SET_INTR, SIG0..SIG7
    sp.status |= Sp::STATUS_BROKE;
    assert!((sp.status & Sp::STATUS_BROKE) != 0);
    sp.write_reg_action(0x00010, 1 << 2).unwrap(); // Clear BROKE
    assert!((sp.status & Sp::STATUS_BROKE) == 0);

    // INTR actions: bit 3 = Clear INTR, bit 4 = Set INTR (both set = no-op)
    let action_clear = sp.write_reg_action(0x00010, 1 << 3).unwrap();
    assert_eq!(action_clear, SpAction::ClearIntr);

    let action_set = sp.write_reg_action(0x00010, 1 << 4).unwrap();
    assert_eq!(action_set, SpAction::SetIntr);

    let action_both = sp.write_reg_action(0x00010, (1 << 3) | (1 << 4)).unwrap();
    assert_eq!(action_both, SpAction::None);

    // SSTEP (bits 5/6) and INTR_ON_BREAK (bits 7/8)
    sp.write_reg_action(0x00010, (1 << 6) | (1 << 8)).unwrap();
    assert!(sp.is_single_step());
    assert!(sp.intr_on_break());
    sp.write_reg_action(0x00010, (1 << 5) | (1 << 7)).unwrap();
    assert!(!sp.is_single_step());
    assert!(!sp.intr_on_break());

    // SIG0 (bits 9/10) through SIG7 (bits 23/24)
    sp.write_reg_action(0x00010, (1 << 10) | (1 << 24)).unwrap();
    assert_ne!(sp.status & Sp::STATUS_SIG0, 0);
    assert_ne!(sp.status & Sp::STATUS_SIG7, 0);
    sp.write_reg_action(0x00010, (1 << 9) | (1 << 23)).unwrap();
    assert_eq!(sp.status & Sp::STATUS_SIG0, 0);
    assert_eq!(sp.status & Sp::STATUS_SIG7, 0);
}

#[test]
fn test_semaphore() {
    let mut sp = Sp::new();
    // Read -> 0 (and sets to 1)
    assert_eq!(sp.read_reg(0x0001C).unwrap(), 0);
    // Subsequent read -> 1
    assert_eq!(sp.read_reg(0x0001C).unwrap(), 1);

    // Write clears semaphore
    sp.write_reg_action(0x0001C, 0).unwrap();
    assert_eq!(sp.read_reg(0x0001C).unwrap(), 0);
}

#[test]
fn test_pc_and_addr_alignment() {
    let mut sp = Sp::new();
    sp.write_reg_action(0x40000, 0xFFFF).unwrap();
    assert_eq!(sp.pc, 0x0FFC);
    assert_eq!(sp.read_reg(0x40000).unwrap(), 0x0FFC);

    sp.write_reg_action(0x00000, 0xFFFF).unwrap();
    assert_eq!(sp.mem_addr, 0x1FF8);
    assert_eq!(sp.read_reg(0x00000).unwrap(), 0x1FF8);

    sp.write_reg_action(0x00004, 0xFFFF_FFFF).unwrap();
    assert_eq!(sp.dram_addr, 0x00FF_FFF8);
    assert_eq!(sp.read_reg(0x00004).unwrap(), 0x00FF_FFF8);
}

#[test]
fn test_dma_rd_wr_2d() {
    let mut sp = Sp::new();
    let mut rdram = Rdram::new(false);
    let mut dmem = vec![0u8; 4096];
    let mut imem = vec![0u8; 4096];

    // Populate RDRAM with 2 rows separated by skip = 16 bytes (stride = 8 + 16 = 24)
    rdram.write_u8(0x1000, 0xAA);
    rdram.write_u8(0x1001, 0xBB);
    rdram.write_u8(0x1018, 0xCC);
    rdram.write_u8(0x1019, 0xDD);

    // len = 7 (8 bytes/row), count = 1 (2 rows), skip = 16 bytes -> (16 << 20) | (1 << 12) | 7
    let len_reg = (16u32 << 20) | (1u32 << 12) | 7u32;

    // Transfer into DMEM starting at 0x0FF8 so row 2 wraps around 4 KiB to 0x0000!
    sp.execute_dma_rd(&rdram, &mut dmem, &mut imem, 0x0FF8, 0x1000, len_reg);

    assert_eq!(dmem[0x0FF8], 0xAA);
    assert_eq!(dmem[0x0FF9], 0xBB);
    assert_eq!(dmem[0x0000], 0xCC);
    assert_eq!(dmem[0x0001], 0xDD);
    assert_eq!(sp.mem_addr, 0x0008);
    assert_eq!(sp.dram_addr, 0x1020);

    // Write back from DMEM (wrapped) into RDRAM at 0x2000 with same skip = 16
    sp.execute_dma_wr(&mut rdram, &dmem, &imem, 0x0FF8, 0x2000, len_reg);

    assert_eq!(rdram.read_u8(0x2000), 0xAA);
    assert_eq!(rdram.read_u8(0x2001), 0xBB);
    assert_eq!(rdram.read_u8(0x2018), 0xCC);
    assert_eq!(rdram.read_u8(0x2019), 0xDD);

    // IMEM bank select (bit 12 = 0x1000) writes into IMEM, never DMEM
    let len_1row = 7u32;
    sp.execute_dma_rd(&rdram, &mut dmem, &mut imem, 0x1000, 0x1000, len_1row);
    assert_eq!(imem[0], 0xAA);
    assert_eq!(imem[1], 0xBB);
    assert_eq!(sp.mem_addr, 0x1008);
}

#[test]
fn test_unknown_offset_traps() {
    let sp = Sp::new();
    assert!(matches!(
        sp.read_reg(0x00FC),
        Err(Trap::UnimplementedRegister {
            device: "SP",
            offset: 0x00FC,
            is_write: false
        })
    ));
    let mut sp_mut = sp;
    assert!(matches!(
        sp_mut.write_reg_action(0x00FC, 0),
        Err(Trap::UnimplementedRegister {
            device: "SP",
            offset: 0x00FC,
            is_write: true
        })
    ));
}
