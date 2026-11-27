//! Cartridge ROM loader: `.z64`/`.v64`/`.n64` endian normalization, header
//! parser, and IPL3 CRC32 CIC detection.
//!
//! Cited: N64brew — ROM Header & Initial Program Load
//!   https://n64brew.dev/wiki/ROM_Header · https://n64brew.dev/wiki/Initial_Program_Load
//! Cross-check: Dragorn421/n64checksum (CC0) IPL3 CRC32 table
//!   https://github.com/Dragorn421/n64checksum · docs/06-cart-cic-pif-saves.md

#[cfg(test)]
mod tests;

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Minimum valid N64 ROM size (64-byte header + 4032-byte IPL3 window).
pub const MIN_ROM_SIZE: usize = 0x1000;

/// Native big-endian `.z64` magic word (`0x80371240`).
pub const MAGIC_Z64: [u8; 4] = [0x80, 0x37, 0x12, 0x40];
/// 16-bit byte-swapped `.v64` magic word.
pub const MAGIC_V64: [u8; 4] = [0x37, 0x80, 0x40, 0x12];
/// 32-bit little-endian `.n64` magic word.
pub const MAGIC_N64: [u8; 4] = [0x40, 0x12, 0x37, 0x80];

/// Source ROM image byte-order format before canonicalization to big-endian `.z64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RomFormat {
    /// Big-endian (native N64 bus order).
    Z64,
    /// 16-bit byte-swapped (Doctor V64).
    V64,
    /// 32-bit little-endian word-swapped.
    N64,
}

impl RomFormat {
    /// Detect byte-order format from the first 4 bytes of a ROM image.
    pub fn detect(bytes: &[u8]) -> Result<Self, RomError> {
        if bytes.len() < MIN_ROM_SIZE {
            return Err(RomError::TooSmall(bytes.len()));
        }
        let magic = [bytes[0], bytes[1], bytes[2], bytes[3]];
        match magic {
            MAGIC_Z64 => Ok(Self::Z64),
            MAGIC_V64 => Ok(Self::V64),
            MAGIC_N64 => Ok(Self::N64),
            other => Err(RomError::UnrecognizedMagic(other)),
        }
    }
}

/// Cartridge CIC lockout chip identified from the IEEE CRC32 of canonical `rom[0x0040..0x1000]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CicType {
    /// CIC-NUS-6101 (NTSC Star Fox 64).
    Cic6101,
    /// CIC-NUS-6102 / 7101 (most common retail & homebrew).
    Cic6102,
    /// CIC-NUS-6103 / 7103.
    Cic6103,
    /// CIC-NUS-6105 / 7105 (includes runtime PIF challenge/response).
    Cic6105,
    /// CIC-NUS-6106 / 7106.
    Cic6106,
    /// CIC-NUS-7102 (PAL Lylat Wars).
    Cic7102,
    /// Non-standard or open-source IPL3 (e.g. libdragon open IPL3) with its raw CRC32.
    Unknown(u32),
}

impl CicType {
    pub const CRC32_6101: u32 = 0x6170_A4A1;
    pub const CRC32_6102: u32 = 0x90BB_6CB5;
    pub const CRC32_6103: u32 = 0x0B05_0EE0;
    pub const CRC32_6105: u32 = 0x98BC_2C86;
    pub const CRC32_6106: u32 = 0xACC8_580A;
    pub const CRC32_7102: u32 = 0x009E_9EA3;

    /// Classify CIC variant from the IEEE CRC32 of bytes `0x0040..0x1000`.
    #[must_use]
    pub const fn from_ipl3_crc32(crc32: u32) -> Self {
        match crc32 {
            Self::CRC32_6101 => Self::Cic6101,
            Self::CRC32_6102 => Self::Cic6102,
            Self::CRC32_6103 => Self::Cic6103,
            Self::CRC32_6105 => Self::Cic6105,
            Self::CRC32_6106 => Self::Cic6106,
            Self::CRC32_7102 => Self::Cic7102,
            other => Self::Unknown(other),
        }
    }

    /// Boot seed byte reported by this CIC during PIF IPL2 handshake (`docs/06` / M2 boot).
    #[must_use]
    pub const fn boot_seed(self) -> u8 {
        match self {
            Self::Cic6101 | Self::Cic6102 | Self::Cic7102 | Self::Unknown(_) => 0x3F,
            Self::Cic6103 => 0x78,
            Self::Cic6105 => 0x91,
            Self::Cic6106 => 0x85,
        }
    }
}

/// Parsed 64-byte N64 cartridge header (`0x0000..0x0040`, canonical big-endian).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomHeader {
    pub pi_dom1_config: u32,
    pub clock_rate: u32,
    pub entry_pc: u32,
    pub release: u32,
    pub crc1: u32,
    pub crc2: u32,
    pub title: String,
    pub media_format: u8,
    pub cart_id: [u8; 2],
    pub region_code: u8,
    pub rom_version: u8,
}

impl RomHeader {
    /// Parse a canonical big-endian 64-byte N64 header slice.
    pub fn parse(rom: &[u8]) -> Result<Self, RomError> {
        if rom.len() < MIN_ROM_SIZE {
            return Err(RomError::TooSmall(rom.len()));
        }
        let read_be32 = |off: usize| -> u32 {
            u32::from_be_bytes([rom[off], rom[off + 1], rom[off + 2], rom[off + 3]])
        };
        let raw_title = &rom[0x20..0x34];
        let title = String::from_utf8_lossy(raw_title)
            .trim_end_matches([' ', '\0'])
            .to_string();

        Ok(Self {
            pi_dom1_config: read_be32(0x00),
            clock_rate: read_be32(0x04),
            entry_pc: read_be32(0x08),
            release: read_be32(0x0C),
            crc1: read_be32(0x10),
            crc2: read_be32(0x14),
            title,
            media_format: rom[0x3B],
            cart_id: [rom[0x3C], rom[0x3D]],
            region_code: rom[0x3E],
            rom_version: rom[0x3F],
        })
    }
}

/// Errors returned while loading or validating an N64 ROM image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RomError {
    TooSmall(usize),
    UnrecognizedMagic([u8; 4]),
    UnalignedLength { format: RomFormat, len: usize },
    Io(String),
}

impl fmt::Display for RomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooSmall(n) => {
                write!(f, "ROM image too small: {n} bytes (minimum {MIN_ROM_SIZE})")
            }
            Self::UnrecognizedMagic(m) => {
                write!(
                    f,
                    "unrecognized N64 ROM magic: [{:02X}, {:02X}, {:02X}, {:02X}]",
                    m[0], m[1], m[2], m[3]
                )
            }
            Self::UnalignedLength { format, len } => write!(
                f,
                "ROM image length {len} is not aligned for {format:?} byte-swapping"
            ),
            Self::Io(msg) => write!(f, "ROM I/O error: {msg}"),
        }
    }
}

impl std::error::Error for RomError {}

/// Standard IEEE 802.3 CRC32 (reflected polynomial `0xEDB8_8320`).
#[must_use]
pub fn crc32_ieee(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Canonicalized, big-endian N64 cartridge image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cart {
    source_format: RomFormat,
    header: RomHeader,
    cic: CicType,
    ipl3_crc32: u32,
    rom: Vec<u8>,
}

impl Cart {
    /// Load, validate, and canonicalize a `.z64`/`.v64`/`.n64` ROM from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, RomError> {
        let source_format = RomFormat::detect(bytes)?;
        let mut rom = bytes.to_vec();
        match source_format {
            RomFormat::Z64 => {}
            RomFormat::V64 => {
                if !rom.len().is_multiple_of(2) {
                    return Err(RomError::UnalignedLength {
                        format: RomFormat::V64,
                        len: rom.len(),
                    });
                }
                for chunk in rom.as_chunks_mut::<2>().0 {
                    chunk.swap(0, 1);
                }
            }
            RomFormat::N64 => {
                if !rom.len().is_multiple_of(4) {
                    return Err(RomError::UnalignedLength {
                        format: RomFormat::N64,
                        len: rom.len(),
                    });
                }
                for chunk in rom.as_chunks_mut::<4>().0 {
                    chunk.reverse();
                }
            }
        }

        let header = RomHeader::parse(&rom)?;
        let ipl3_crc32 = crc32_ieee(&rom[0x0040..0x1000]);
        let cic = CicType::from_ipl3_crc32(ipl3_crc32);

        Ok(Self {
            source_format,
            header,
            cic,
            ipl3_crc32,
            rom,
        })
    }

    /// Load a cartridge image from disk.
    pub fn from_file(path: &Path) -> Result<Self, RomError> {
        let bytes = std::fs::read(path).map_err(|e| RomError::Io(e.to_string()))?;
        Self::from_bytes(&bytes)
    }

    #[must_use]
    pub fn source_format(&self) -> RomFormat {
        self.source_format
    }

    #[must_use]
    pub fn header(&self) -> &RomHeader {
        &self.header
    }

    #[must_use]
    pub fn cic(&self) -> CicType {
        self.cic
    }

    #[must_use]
    pub fn ipl3_crc32(&self) -> u32 {
        self.ipl3_crc32
    }

    #[must_use]
    pub fn rom(&self) -> &[u8] {
        &self.rom
    }
}
