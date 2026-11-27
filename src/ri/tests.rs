//! RDRAM Interface (RI) unit tests (`src/ri/tests.rs`).
//!
//! Cited: N64brew — RDRAM Interface
//!   https://n64brew.dev/wiki/RDRAM_Interface · docs/02-bus-memory-dma.md §5

use super::*;

#[test]
fn test_ri_select_default() {
    let ri = Ri::new();
    assert_eq!(ri.select, 0x14);
    assert_eq!(ri.read_reg(0x0C).unwrap(), 0x14);
}

#[test]
fn test_ri_register_rw_masking() {
    let mut ri = Ri::new();
    ri.write_reg(0x00, 0xFF).unwrap();
    assert_eq!(ri.read_reg(0x00).unwrap(), 0x0F);

    ri.write_reg(0x04, 0xFF).unwrap();
    assert_eq!(ri.read_reg(0x04).unwrap(), 0x7F);

    ri.write_reg(0x0C, 0xFF).unwrap();
    assert_eq!(ri.read_reg(0x0C).unwrap(), 0x1F);
}

#[test]
fn test_ri_unimplemented_trap() {
    let ri = Ri::new();
    assert!(matches!(
        ri.read_reg(0x20),
        Err(Trap::UnimplementedRegister {
            device: "RI",
            offset: 0x20,
            is_write: false
        })
    ));
}
