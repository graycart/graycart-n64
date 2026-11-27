//! Cartridge ROM loader shell (full endian normalization, header parse, and CIC
//! detection land in Task M0.4).
//!
//! Cited: N64brew — ROM Header
//!   https://n64brew.dev/wiki/ROM_Header · docs/06-cart-cic-pif-saves.md

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RomFormat {
    Z64,
    V64,
    N64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CicType {
    Cic6101,
    Cic6102,
    Cic6103,
    Cic6105,
    Cic6106,
    Cic7102,
    Unknown(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomHeader {
    pub entry_pc: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RomError {
    TooSmall(usize),
    UnrecognizedMagic([u8; 4]),
}

impl fmt::Display for RomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooSmall(n) => write!(f, "ROM image too small ({n} bytes)"),
            Self::UnrecognizedMagic(m) => write!(f, "unrecognized ROM magic {m:02X?}"),
        }
    }
}

impl std::error::Error for RomError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cart {
    header: RomHeader,
    rom: Vec<u8>,
}

impl Cart {
    #[must_use]
    pub fn header(&self) -> &RomHeader {
        &self.header
    }

    #[must_use]
    pub fn rom(&self) -> &[u8] {
        &self.rom
    }
}
