//! NEC VR4300 / N64 MIPS III tests for SI.
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual U10504EJ7V0UMJ1 and docs/01-cpu-r4300i.md.

use super::*;

#[test]
fn test_si_dram_addr_alignment() {
    let mut si = Si::new();
    assert_eq!(si.write_reg_action(0x00, 0x1234_56FF), Ok(SiAction::None));
    assert_eq!(si.dram_addr, 0x0034_56F8);
    assert_eq!(si.read_reg(0x00), Ok(0x0034_56F8));
}

#[test]
fn test_si_dma_actions() {
    let mut si = Si::new();
    si.dram_addr = 0x1000;

    assert_eq!(
        si.write_reg_action(0x04, 0),
        Ok(SiAction::DmaPifToRdram {
            dram_addr: 0x1000,
            delay_cycles: 4096
        })
    );
    assert_eq!(si.status & Si::DMA_BUSY, Si::DMA_BUSY);

    si.complete_dma();
    assert_eq!(si.status & Si::DMA_BUSY, 0);
    assert_eq!(si.status & Si::INTR, Si::INTR);

    assert_eq!(
        si.write_reg_action(0x10, 0),
        Ok(SiAction::DmaRdramToPif {
            dram_addr: 0x1000,
            delay_cycles: 4096
        })
    );
}

#[test]
fn test_si_status_clear_intr() {
    let mut si = Si::new();
    si.status = Si::INTR;
    assert_eq!(si.read_reg(0x18), Ok(Si::INTR));

    assert_eq!(
        si.write_reg_action(0x18, 0xFFFF_FFFF),
        Ok(SiAction::ClearIntr)
    );
    assert_eq!(si.read_reg(0x18), Ok(0));
}

#[test]
fn test_si_unimplemented_registers() {
    let si = Si::new();
    let mut si_mut = Si::new();

    assert!(si.read_reg(0x04).is_err());
    assert!(si_mut.write_reg(0x20, 0).is_err());
}
