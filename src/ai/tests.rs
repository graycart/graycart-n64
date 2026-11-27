//! NEC VR4300, VR4305, VR4310 User's Manual U10504EJ7V0UMJ1 and docs/01-cpu-r4300i.md.
//! Unit tests for Audio Interface (AI).

use super::*;

#[test]
fn test_ai_fifo_and_dma() {
    let mut ai = Ai::new();

    // Initial status: not busy, not full, enable 0
    let status = ai.read_reg(0x0C).unwrap();
    assert_eq!(status & (1 << 30), 0); // busy = 0
    assert_eq!(status & (1 << 31), 0); // full = 0

    // Write DRAM addr
    ai.write_reg(0x00, 0x1000).unwrap();
    assert_eq!(ai.dram_addr, 0x1000);

    // Write LEN (triggers 1st DMA)
    let action = ai.write_reg_action(0x04, 0x100).unwrap();
    assert!(matches!(action, AiAction::StartDma { .. }));
    assert_eq!(ai.fifo_count, 1);

    // Status now busy (bit 30), not full
    let status1 = ai.read_reg(0x0C).unwrap();
    assert_ne!(status1 & (1 << 30), 0);
    assert_eq!(status1 & (1 << 31), 0);

    // Write 2nd buffer (triggers 2nd DMA, FIFO full)
    ai.write_reg(0x00, 0x2000).unwrap();
    let action2 = ai.write_reg_action(0x04, 0x100).unwrap();
    assert_eq!(action2, AiAction::None);
    assert_eq!(ai.fifo_count, 2);

    let status2 = ai.read_reg(0x0C).unwrap();
    assert_ne!(status2 & (1 << 31), 0); // full = 1

    // Reading non-status registers returns fifo_len[0]
    assert_eq!(ai.read_reg(0x00).unwrap(), 0x100);

    // Write status clears interrupt
    let stat_action = ai.write_reg_action(0x0C, 0).unwrap();
    assert_eq!(stat_action, AiAction::ClearIntr);

    // Dacrate & bitrate & control
    ai.write_reg(0x10, 0x100).unwrap();
    assert_eq!(ai.dacrate, 0x100);
    ai.write_reg(0x14, 5).unwrap();
    assert_eq!(ai.bitrate, 5);
    ai.write_reg(0x08, 1).unwrap();
    assert_eq!(ai.control, 1);

    // on_dma_complete pops slot 0, shifts slot 1
    let next_dma = ai.on_dma_complete();
    assert!(next_dma.is_some());
    assert_eq!(ai.fifo_count, 1);

    let final_dma = ai.on_dma_complete();
    assert!(final_dma.is_none());
    assert_eq!(ai.fifo_count, 0);

    // Unimplemented offsets trap
    assert!(matches!(
        ai.read_reg(0x20),
        Err(Trap::UnimplementedRegister {
            device: "AI",
            offset: 0x20,
            is_write: false
        })
    ));
    assert!(matches!(
        ai.write_reg(0x20, 0),
        Err(Trap::UnimplementedRegister {
            device: "AI",
            offset: 0x20,
            is_write: true
        })
    ));
}
