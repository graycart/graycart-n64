//! ISViewer 64 cartridge debug console (`0x13FF_0000..=0x13FF_FFFF`).
//!
//! Used by `n64-systemtest`, `nemu64-test`, and `libdragon` for headless
//! UTF-8 diagnostic output without requiring a framebuffer or VI scan-out.
//!
//! Cited: libdragon `debug.c` (Unlicense) & N64brew ISViewer register layout
//!   https://github.com/DragonMinded/libdragon · https://n64brew.dev/wiki/Memory_map

use serde::{Deserialize, Serialize};

/// Base physical address of the ISViewer 64 debug window.
pub const ISVIEWER_BASE: u32 = 0x13FF_0000;
/// Inclusive end physical address of the ISViewer 64 debug window.
pub const ISVIEWER_END: u32 = 0x13FF_FFFF;
/// Magic token (`"IS64"`) returned at offset `0x0000`.
pub const ISVIEWER_MAGIC: u32 = 0x4953_3634;
/// Offset where the character ring buffer begins.
pub const ISVIEWER_BUF_OFFSET: usize = 0x0020;
/// Usable ring buffer capacity (`0x10000 - 0x20 = 0xFFE0` bytes).
pub const ISVIEWER_BUF_SIZE: usize = 0x1_0000 - ISVIEWER_BUF_OFFSET;

/// ISViewer 64 debug capture peripheral.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsViewer {
    /// Magic token register (`0x0000`). Defaults to `ISVIEWER_MAGIC` (`"IS64"`).
    magic: u32,
    /// Ring buffer read pointer (`0x0004`).
    read_ptr: u32,
    /// Ring buffer write pointer (`0x0014`).
    write_ptr: u32,
    /// 64 KiB minus 32 B character ring buffer (`0x0020..=0xFFFF`).
    buffer: Vec<u8>,
    /// Captured output stream drained on every `write_ptr` commit.
    output: Vec<u8>,
}

impl IsViewer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            magic: ISVIEWER_MAGIC,
            read_ptr: 0,
            write_ptr: 0,
            buffer: vec![0; ISVIEWER_BUF_SIZE],
            output: Vec::new(),
        }
    }

    pub fn clear_output(&mut self) {
        self.output.clear();
    }

    #[must_use]
    pub fn output(&self) -> &[u8] {
        &self.output
    }

    #[must_use]
    pub fn output_lossy(&self) -> String {
        String::from_utf8_lossy(&self.output).into_owned()
    }

    #[must_use]
    pub fn read_u32(&self, offset: u32) -> u32 {
        let off = (offset & 0xFFFF) as usize;
        match off {
            0x0000 => self.magic,
            0x0004 => self.read_ptr,
            0x0014 => self.write_ptr,
            o if o >= ISVIEWER_BUF_OFFSET && o + 4 <= 0x1_0000 => {
                let idx = o - ISVIEWER_BUF_OFFSET;
                u32::from_be_bytes([
                    self.buffer[idx],
                    self.buffer[idx + 1],
                    self.buffer[idx + 2],
                    self.buffer[idx + 3],
                ])
            }
            _ => 0,
        }
    }

    pub fn write_u32(&mut self, offset: u32, val: u32) {
        let off = (offset & 0xFFFF) as usize;
        match off {
            0x0000 => self.magic = val,
            0x0004 => self.read_ptr = val % (ISVIEWER_BUF_SIZE as u32),
            0x0014 => {
                self.write_ptr = val % (ISVIEWER_BUF_SIZE as u32);
                self.flush_ring();
            }
            o if o >= ISVIEWER_BUF_OFFSET && o + 4 <= 0x1_0000 => {
                let idx = o - ISVIEWER_BUF_OFFSET;
                self.buffer[idx..idx + 4].copy_from_slice(&val.to_be_bytes());
            }
            _ => {}
        }
    }

    pub fn write_u8(&mut self, offset: u32, val: u8) {
        let off = (offset & 0xFFFF) as usize;
        if (ISVIEWER_BUF_OFFSET..0x1_0000).contains(&off) {
            self.buffer[off - ISVIEWER_BUF_OFFSET] = val;
        }
    }

    #[must_use]
    pub fn read_u8(&self, offset: u32) -> u8 {
        let off = (offset & 0xFFFF) as usize;
        if (ISVIEWER_BUF_OFFSET..0x1_0000).contains(&off) {
            self.buffer[off - ISVIEWER_BUF_OFFSET]
        } else {
            let word = self.read_u32((off & !3) as u32);
            let shift = (3 - (off & 3)) * 8;
            ((word >> shift) & 0xFF) as u8
        }
    }

    fn flush_ring(&mut self) {
        let mut cursor = self.read_ptr as usize;
        let target = self.write_ptr as usize;
        while cursor != target {
            self.output.push(self.buffer[cursor]);
            cursor = (cursor + 1) % ISVIEWER_BUF_SIZE;
        }
        self.read_ptr = self.write_ptr;
    }
}

impl Default for IsViewer {
    fn default() -> Self {
        Self::new()
    }
}
