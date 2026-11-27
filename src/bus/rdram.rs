//! N64 RDRAM with bit-packed 9th-bit (hidden-bit) side array.
//!
//! Each byte of N64 RDRAM physically carries a 9th bit used by the RDP for
//! anti-aliased coverage and Z-buffer metadata. Storing it from M0 ensures
//! hardware-exact software (`CpuRdpLle`) and Vulkan compute (`EdgeWalker`)
//! frame hashes are structurally possible without retrofitting memory paths.
//!
//! Cited: N64brew — Memory map & RDRAM Interface
//!   https://n64brew.dev/wiki/Memory_map · docs/02-bus-memory-dma.md
//! Cross-check: Graycart N64 Development Plan §3.5 & §3.8

use serde::{Deserialize, Serialize};

use crate::trap::Trap;

/// RDRAM backing store (4 MiB base or 8 MiB with Expansion Pak) plus a
/// bit-packed 9th-bit side array (1 bit per RDRAM byte).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rdram {
    /// Big-endian RDRAM data bytes.
    data: Vec<u8>,
    /// Bit-packed 9th bit per byte (`hidden_bits[addr >> 3] & (1 << (addr & 7))`).
    hidden_bits: Vec<u8>,
}

impl Rdram {
    /// Standard console RDRAM size without Expansion Pak (4 MiB).
    pub const BASE_SIZE: usize = 4 * 1024 * 1024;
    /// Console RDRAM size with Expansion Pak installed (8 MiB).
    pub const EXPANSION_SIZE: usize = 8 * 1024 * 1024;

    #[must_use]
    pub fn new(expansion_pak: bool) -> Self {
        let size = if expansion_pak {
            Self::EXPANSION_SIZE
        } else {
            Self::BASE_SIZE
        };
        Self {
            data: vec![0; size],
            hidden_bits: vec![0; size / 8],
        }
    }

    /// Clear all RDRAM bytes and 9th bits to zero.
    pub fn clear(&mut self) {
        self.data.fill(0);
        self.hidden_bits.fill(0);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[must_use]
    pub fn hidden_bits_len(&self) -> usize {
        self.hidden_bits.len()
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Read the 9th (hidden) bit for byte offset `addr`.
    ///
    /// Addresses past installed RDRAM return `false` (open bank).
    #[must_use]
    pub fn read_hidden_bit(&self, addr: usize) -> bool {
        if addr >= self.data.len() {
            return false;
        }
        (self.hidden_bits[addr >> 3] & (1 << (addr & 7))) != 0
    }

    /// Write the 9th (hidden) bit for byte offset `addr`.
    ///
    /// Writes past installed RDRAM are ignored (open bank).
    pub fn write_hidden_bit(&mut self, addr: usize, bit: bool) {
        if addr >= self.data.len() {
            return;
        }
        let mask = 1u8 << (addr & 7);
        if bit {
            self.hidden_bits[addr >> 3] |= mask;
        } else {
            self.hidden_bits[addr >> 3] &= !mask;
        }
    }

    /// Read a byte from RDRAM. Reads above installed capacity (e.g. >8 MiB) return `0`.
    #[must_use]
    pub fn read_u8(&self, pa: u32) -> u8 {
        let idx = pa as usize;
        self.data.get(idx).copied().unwrap_or(0)
    }

    /// Write a byte to RDRAM. Writes above installed capacity are ignored.
    pub fn write_u8(&mut self, pa: u32, val: u8) {
        let idx = pa as usize;
        if let Some(slot) = self.data.get_mut(idx) {
            *slot = val;
        }
    }

    /// Read a big-endian 16-bit halfword from RDRAM.
    pub fn read_u16(&self, pa: u32) -> Result<u16, Trap> {
        if pa & 1 != 0 {
            return Err(Trap::MisalignedAccess { pa, size: 2 });
        }
        let idx = pa as usize;
        if idx + 2 > self.data.len() {
            return Ok(0);
        }
        Ok(u16::from_be_bytes([self.data[idx], self.data[idx + 1]]))
    }

    /// Write a big-endian 16-bit halfword to RDRAM.
    pub fn write_u16(&mut self, pa: u32, val: u16) -> Result<(), Trap> {
        if pa & 1 != 0 {
            return Err(Trap::MisalignedAccess { pa, size: 2 });
        }
        let idx = pa as usize;
        if idx + 2 <= self.data.len() {
            let bytes = val.to_be_bytes();
            self.data[idx..idx + 2].copy_from_slice(&bytes);
        }
        Ok(())
    }

    /// Read a big-endian 32-bit word from RDRAM.
    pub fn read_u32(&self, pa: u32) -> Result<u32, Trap> {
        if pa & 3 != 0 {
            return Err(Trap::MisalignedAccess { pa, size: 4 });
        }
        let idx = pa as usize;
        if idx + 4 > self.data.len() {
            return Ok(0);
        }
        Ok(u32::from_be_bytes([
            self.data[idx],
            self.data[idx + 1],
            self.data[idx + 2],
            self.data[idx + 3],
        ]))
    }

    /// Write a big-endian 32-bit word to RDRAM.
    pub fn write_u32(&mut self, pa: u32, val: u32) -> Result<(), Trap> {
        if pa & 3 != 0 {
            return Err(Trap::MisalignedAccess { pa, size: 4 });
        }
        let idx = pa as usize;
        if idx + 4 <= self.data.len() {
            let bytes = val.to_be_bytes();
            self.data[idx..idx + 4].copy_from_slice(&bytes);
        }
        Ok(())
    }

    /// Read a big-endian 64-bit doubleword from RDRAM.
    pub fn read_u64(&self, pa: u32) -> Result<u64, Trap> {
        if pa & 7 != 0 {
            return Err(Trap::MisalignedAccess { pa, size: 8 });
        }
        let idx = pa as usize;
        if idx + 8 > self.data.len() {
            return Ok(0);
        }
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&self.data[idx..idx + 8]);
        Ok(u64::from_be_bytes(buf))
    }

    /// Write a big-endian 64-bit doubleword to RDRAM.
    pub fn write_u64(&mut self, pa: u32, val: u64) -> Result<(), Trap> {
        if pa & 7 != 0 {
            return Err(Trap::MisalignedAccess { pa, size: 8 });
        }
        let idx = pa as usize;
        if idx + 8 <= self.data.len() {
            self.data[idx..idx + 8].copy_from_slice(&val.to_be_bytes());
        }
        Ok(())
    }
}
