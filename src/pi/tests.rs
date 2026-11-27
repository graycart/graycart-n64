//! Unit tests for Peripheral Interface (PI).
//!
//! Cited: N64brew — Peripheral Interface
//!   https://n64brew.dev/wiki/Peripheral_Interface · docs/02-bus-memory-dma.md §6

use super::*;

#[test]
fn test_pi_address_masking() {
    let mut pi = Pi::new();
    pi.write_reg(0x00, 0x01FF_FFFF).unwrap();
    assert_eq!(pi.read_reg(0x00).unwrap(), 0x00FF_FFFE);

    pi.write_reg(0x04, 0xFFFF_FFFF).unwrap();
    assert_eq!(pi.read_reg(0x04).unwrap(), 0xFFFF_FFFE);
}

#[test]
fn test_pi_wr_len_dma() {
    let mut pi = Pi::new();
    pi.write_reg(0x00, 0x1000).unwrap();
    pi.write_reg(0x04, 0x1000_0000).unwrap();

    let action = pi.write_reg_action(0x0C, 0xFF).unwrap();
    match action {
        PiAction::StartCartToRdram {
            dram_addr,
            cart_addr,
            len,
            delay_cycles,
        } => {
            assert_eq!(dram_addr, 0x1000);
            assert_eq!(cart_addr, 0x1000_0000);
            assert_eq!(len, 256);
            assert!(delay_cycles >= 64);
        }
        other => panic!("Unexpected action: {:?}", other),
    }

    assert_ne!(pi.status & Pi::DMA_BUSY, 0);
    assert_eq!(pi.dma_count, 1);
    assert_eq!(pi.dma_bytes_total, 256);

    pi.complete_dma(256);
    assert_eq!(pi.status & Pi::DMA_BUSY, 0);
    assert_ne!(pi.status & Pi::INTR, 0);
}

#[test]
fn test_pi_status_writes() {
    let mut pi = Pi::new();
    pi.status = Pi::DMA_BUSY | Pi::ERROR | Pi::INTR;

    let action = pi.write_reg_action(0x10, 0x02).unwrap();
    assert_eq!(action, PiAction::ClearIntr);
    assert_eq!(pi.status & Pi::INTR, 0);
    assert_ne!(pi.status & Pi::DMA_BUSY, 0);

    let action = pi.write_reg_action(0x10, 0x01).unwrap();
    assert_eq!(action, PiAction::Reset);
    assert_eq!(pi.status & (Pi::DMA_BUSY | Pi::ERROR), 0);
}

#[test]
fn test_bsd_domain_registers() {
    let mut pi = Pi::new();
    pi.write_reg(0x14, 0xFFAB).unwrap();
    assert_eq!(pi.read_reg(0x14).unwrap(), 0xAB);

    pi.write_reg(0x18, 0xFFCD).unwrap();
    assert_eq!(pi.read_reg(0x18).unwrap(), 0xCD);

    pi.write_reg(0x1C, 0xFFEF).unwrap();
    assert_eq!(pi.read_reg(0x1C).unwrap(), 0x0F);

    pi.write_reg(0x20, 0xFFFF).unwrap();
    assert_eq!(pi.read_reg(0x20).unwrap(), 0x03);

    pi.write_reg(0x24, 0x1234).unwrap();
    assert_eq!(pi.read_reg(0x24).unwrap(), 0x34);

    pi.write_reg(0x28, 0x5678).unwrap();
    assert_eq!(pi.read_reg(0x28).unwrap(), 0x78);

    pi.write_reg(0x2C, 0x9ABC).unwrap();
    assert_eq!(pi.read_reg(0x2C).unwrap(), 0x0C);

    pi.write_reg(0x30, 0xDEF0).unwrap();
    assert_eq!(pi.read_reg(0x30).unwrap(), 0x00);
}

#[test]
fn test_unknown_pi_offsets() {
    let pi = Pi::new();
    assert!(matches!(
        pi.read_reg(0x34),
        Err(Trap::UnimplementedRegister {
            device: "PI",
            offset: 0x34,
            is_write: false
        })
    ));

    let mut pi_mut = Pi::new();
    assert!(matches!(
        pi_mut.write_reg(0x34, 0),
        Err(Trap::UnimplementedRegister {
            device: "PI",
            offset: 0x34,
            is_write: true
        })
    ));
}

#[test]
fn test_rd_len_dma() {
    let mut pi = Pi::new();
    let action = pi.write_reg_action(0x08, 0x03).unwrap();
    match action {
        PiAction::StartRdramToCart { len, .. } => {
            assert_eq!(len, 4);
        }
        other => panic!("Unexpected action: {:?}", other),
    }
}
