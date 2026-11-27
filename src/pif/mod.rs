//! PIF-NUS HLE shell (`0x1FC0_0000..=0x1FC0_07FF`, no proprietary PIF/CIC dumps).
//!
//! Cited: N64brew — PIF-NUS
//!   https://n64brew.dev/wiki/PIF-NUS · docs/06-cart-cic-pif-saves.md

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::cart::CicType;

/// Size of PIF RAM (`0x1FC0_07C0..=0x1FC0_07FF`).
pub const PIF_RAM_SIZE: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pif {
    /// 64-byte PIF RAM window.
    pub ram: Vec<u8>,
    pub boot_terminated: bool,
    pub rom_locked: bool,
}

impl Pif {
    #[must_use]
    pub fn new() -> Self {
        Self {
            ram: vec![0; PIF_RAM_SIZE],
            boot_terminated: false,
            rom_locked: false,
        }
    }

    #[must_use]
    pub const fn cic_seed(cic: CicType) -> u8 {
        match cic {
            CicType::Cic6101 | CicType::Cic6102 | CicType::Cic7102 | CicType::Unknown(_) => 0x3F,
            CicType::Cic6103 => 0x78,
            CicType::Cic6105 => 0x91,
            CicType::Cic6106 => 0x85,
        }
    }

    pub fn seed_boot_ram(&mut self, cic: CicType) {
        self.ram.fill(0);
        self.boot_terminated = false;
        self.rom_locked = false;
        let seed = Self::cic_seed(cic);
        // Standard N64 PIF boot status word in big-endian at ram[0x24..0x28]: [0x00, 0x04, seed, 0x3F]
        if self.ram.len() >= 0x28 {
            self.ram[0x24] = 0x00;
            self.ram[0x25] = 0x04;
            self.ram[0x26] = seed;
            self.ram[0x27] = 0x3F;
        }
    }

    pub fn process_write_command(&mut self) {
        if self.ram.is_empty() {
            return;
        }
        let last_idx = self.ram.len() - 1;
        let ctrl = self.ram[last_idx];

        if (ctrl & 0x08) != 0 {
            self.boot_terminated = true;
            self.ram[last_idx] &= !0x08;
        }
        if (ctrl & 0x10) != 0 {
            self.rom_locked = true;
            self.ram[last_idx] &= !0x10;
        }
        if (ctrl & 0x20) != 0 {
            self.ram[last_idx] = (self.ram[last_idx] & !0x20) | 0x80;
        }
        if (ctrl & 0x40) != 0 {
            self.ram[..last_idx].fill(0);
            self.ram[last_idx] &= !0x40;
        }
    }

    pub fn execute_joybus(&mut self) {
        let mut i = 0;
        let mut ch = 0;
        let len = self.ram.len();

        while i < len - 1 && ch < 4 {
            let b = self.ram[i];
            if b == 0x00 {
                ch += 1;
                i += 1;
                continue;
            }
            if b == 0xFF || b == 0xFD {
                i += 1;
                continue;
            }
            if b == 0xFE {
                break;
            }

            let tx = (b & 0x3F) as usize;
            if i + 1 >= len {
                break;
            }
            let rx = (self.ram[i + 1] & 0x3F) as usize;

            if i + 2 + tx + rx > len {
                break;
            }

            if ch == 0 {
                if tx >= 1 && i + 2 < len {
                    let cmd = self.ram[i + 2];
                    let rx_start = i + 2 + tx;
                    match cmd {
                        0x00 | 0xFF => {
                            // Info / Reset
                            if rx >= 3 && rx_start + 3 <= len {
                                self.ram[rx_start] = 0x05;
                                self.ram[rx_start + 1] = 0x00;
                                self.ram[rx_start + 2] = 0x02;
                            }
                        }
                        0x01 => {
                            // Read controller state
                            if rx >= 4 && rx_start + 4 <= len {
                                self.ram[rx_start] = 0x00;
                                self.ram[rx_start + 1] = 0x00;
                                self.ram[rx_start + 2] = 0x00;
                                self.ram[rx_start + 3] = 0x00;
                            }
                        }
                        _ => {
                            self.ram[i + 1] |= 0x80;
                        }
                    }
                }
            } else {
                // Unconnected channels 1..3
                self.ram[i + 1] |= 0x80;
            }

            i += 2 + tx + rx;
            ch += 1;
        }

        if !self.ram.is_empty() {
            let last_idx = self.ram.len() - 1;
            self.ram[last_idx] &= !0x01;
        }
    }
}

impl Default for Pif {
    fn default() -> Self {
        Self::new()
    }
}
