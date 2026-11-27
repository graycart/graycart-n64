//! PIF-NUS HLE shell (`0x1FC0_0000..=0x1FC0_07FF`, no proprietary PIF/CIC dumps).
//!
//! Cited: N64brew — PIF-NUS
//!   https://n64brew.dev/wiki/PIF-NUS · docs/06-cart-cic-pif-saves.md
//! Note: Boot-critical CIC/PIF/terminate-boot HLE lands in M2; full Joybus,
//! 6105 runtime challenge, saves, and 4-port Transfer Pak land in M8.

use serde::{Deserialize, Serialize};

/// Size of PIF RAM (`0x1FC0_07C0..=0x1FC0_07FF`).
pub const PIF_RAM_SIZE: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pif {
    /// 64-byte PIF RAM window.
    pub ram: Vec<u8>,
}

impl Pif {
    #[must_use]
    pub fn new() -> Self {
        Self {
            ram: vec![0; PIF_RAM_SIZE],
        }
    }
}

impl Default for Pif {
    fn default() -> Self {
        Self::new()
    }
}
