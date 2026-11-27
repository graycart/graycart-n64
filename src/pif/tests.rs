//! NEC VR4300 / N64 MIPS III tests for PIF.
//! Cited: NEC VR4300, VR4305, VR4310 User's Manual U10504EJ7V0UMJ1 and docs/01-cpu-r4300i.md.

use super::*;
use crate::cart::CicType;

#[test]
fn test_cic_seeds() {
    assert_eq!(Pif::cic_seed(CicType::Cic6102), 0x3F);
    assert_eq!(Pif::cic_seed(CicType::Cic6103), 0x78);
    assert_eq!(Pif::cic_seed(CicType::Cic6105), 0x91);
    assert_eq!(Pif::cic_seed(CicType::Cic6106), 0x85);
}

#[test]
fn test_seed_boot_ram() {
    let mut pif = Pif::new();
    pif.seed_boot_ram(CicType::Cic6103);
    assert_eq!(pif.ram[0x26], 0x78);
    assert_eq!(pif.ram[0x27], 0x3F);
}

#[test]
fn test_process_write_command_flags() {
    let mut pif = Pif::new();
    let last = pif.ram.len() - 1;

    // Terminate boot (0x08)
    pif.ram[last] = 0x08;
    pif.process_write_command();
    assert!(pif.boot_terminated);
    assert_eq!(pif.ram[last] & 0x08, 0);

    // Lock ROM (0x10)
    pif.ram[last] = 0x10;
    pif.process_write_command();
    assert!(pif.rom_locked);
    assert_eq!(pif.ram[last] & 0x10, 0);

    // Ack bit (0x20) -> sets 0x80
    pif.ram[last] = 0x20;
    pif.process_write_command();
    assert_eq!(pif.ram[last] & 0x80, 0x80);
    assert_eq!(pif.ram[last] & 0x20, 0);

    // Clear RAM (0x40)
    pif.ram[0] = 0xAB;
    pif.ram[last] = 0x40;
    pif.process_write_command();
    assert_eq!(pif.ram[0], 0x00);
    assert_eq!(pif.ram[last] & 0x40, 0);
}

#[test]
fn test_execute_joybus_channel0_and_unconnected() {
    let mut pif = Pif::new();

    // Channel 0: tx=1, rx=3, cmd=0x00 (Info)
    pif.ram[0] = 0x01; // tx = 1
    pif.ram[1] = 0x03; // rx = 3
    pif.ram[2] = 0x00; // cmd = Info

    // Channel 1 (starts at i = 6): tx=1, rx=1, cmd=0x01 (unconnected -> error bit 0x80)
    let idx1 = 6;
    pif.ram[idx1] = 0x01;
    pif.ram[idx1 + 1] = 0x01;
    pif.ram[idx1 + 2] = 0x01;

    // End of block
    let idx2 = idx1 + 1 + 1 + 1; // 9
    pif.ram[idx2] = 0xFE;

    pif.execute_joybus();

    // Channel 0 response
    assert_eq!(pif.ram[3], 0x05);
    assert_eq!(pif.ram[4], 0x00);
    assert_eq!(pif.ram[5], 0x02);

    // Channel 1 error bit set
    assert_eq!(pif.ram[idx1 + 1] & 0x80, 0x80);
}
