//! RSP (SP) MMIO register file and 2D DMA engine (`0x0404_0000..=0x040B_FFFF`).
//!
//! Cited: N64brew — Reality Signal Processor (Interface & DMA)
//!   https://n64brew.dev/wiki/Reality_Signal_Processor/Interface · docs/03-rsp.md §3
//!
//! Offset convention passed by [`crate::bus::Bus`]: `offset = pa - 0x0404_0000`.
//! - `0x00000` (`0x0404_0000`): `SP_MEM_ADDR` (bits 0..11 addr & 0xFF8, bit 12 = 0 DMEM / 1 IMEM)
//! - `0x00004` (`0x0404_0004`): `SP_DRAM_ADDR` (bits 0..23 & 0x00FF_FFF8)
//! - `0x00008` (`0x0404_0008`): `SP_RD_LEN` (RDRAM -> DMEM/IMEM)
//! - `0x0000C` (`0x0404_000C`): `SP_WR_LEN` (DMEM/IMEM -> RDRAM)
//! - `0x00010` (`0x0404_0010`): `SP_STATUS` (read flags / write 25-bit paired clear/set)
//! - `0x00014` (`0x0404_0014`): `SP_DMA_FULL`
//! - `0x00018` (`0x0404_0018`): `SP_DMA_BUSY`
//! - `0x0001C` (`0x0404_001C`): `SP_SEMAPHORE` (read atomically returns old & sets 1; write clears 0)
//! - `0x40000` (`0x0408_0000`): `SP_PC` (12-bit IMEM PC `& 0xFFC`)
//! - `0x40004` (`0x0408_0004`): `SP_IBIST` (7-bit BIST latch)

#[cfg(test)]
#[path = "regs_tests.rs"]
mod tests;

use std::cell::Cell;

use serde::{Deserialize, Serialize};

use crate::bus::Rdram;
use crate::trap::Trap;

/// Actions returned by `SP` register writes for [`crate::bus::Bus`] to execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpAction {
    None,
    /// RDRAM -> SP DMEM/IMEM 2D DMA (`SP_RD_LEN`).
    StartDmaRd {
        mem_addr: u32,
        dram_addr: u32,
        len_reg: u32,
        delay_cycles: u64,
    },
    /// SP DMEM/IMEM -> RDRAM 2D DMA (`SP_WR_LEN`).
    StartDmaWr {
        mem_addr: u32,
        dram_addr: u32,
        len_reg: u32,
        delay_cycles: u64,
    },
    /// Clear `MI_INTR_SP`.
    ClearIntr,
    /// Assert `MI_INTR_SP`.
    SetIntr,
}

/// SP MMIO register file and DMA state (single source of truth shared by CPU MMIO & RSP COP0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sp {
    /// `SP_MEM_ADDR` (`& 0x1FF8`: bit 12 = IMEM/DMEM bank, bits 11..3 = 8-byte aligned offset).
    pub mem_addr: u32,
    /// `SP_DRAM_ADDR` (`& 0x00FF_FFF8`).
    pub dram_addr: u32,
    /// Last `SP_RD_LEN` / `SP_WR_LEN` register value.
    pub dma_len: u32,
    /// `SP_STATUS` flag register (power-on default = `STATUS_HALT`).
    pub status: u32,
    /// `SP_SEMAPHORE` atomic test-and-set flag.
    pub semaphore: Cell<bool>,
    /// `SP_PC` 12-bit program counter (`& 0xFFC`).
    pub pc: u16,
    /// `SP_IBIST` 7-bit register.
    pub ibist: u32,
    /// Telemetry: total SP DMA transfers triggered.
    pub dma_count: u64,
    /// Telemetry: total SP DMA bytes transferred.
    pub dma_bytes_total: u64,
}

impl Sp {
    pub const STATUS_HALT: u32 = 1 << 0;
    pub const STATUS_BROKE: u32 = 1 << 1;
    pub const STATUS_DMA_BUSY: u32 = 1 << 2;
    pub const STATUS_DMA_FULL: u32 = 1 << 3;
    pub const STATUS_IO_FULL: u32 = 1 << 4;
    pub const STATUS_SSTEP: u32 = 1 << 5;
    pub const STATUS_INTR_ON_BREAK: u32 = 1 << 6;
    pub const STATUS_SIG0: u32 = 1 << 7;
    pub const STATUS_SIG1: u32 = 1 << 8;
    pub const STATUS_SIG2: u32 = 1 << 9;
    pub const STATUS_SIG3: u32 = 1 << 10;
    pub const STATUS_SIG4: u32 = 1 << 11;
    pub const STATUS_SIG5: u32 = 1 << 12;
    pub const STATUS_SIG6: u32 = 1 << 13;
    pub const STATUS_SIG7: u32 = 1 << 14;

    #[must_use]
    pub fn new() -> Self {
        Self {
            mem_addr: 0,
            dram_addr: 0,
            dma_len: 0,
            status: Self::STATUS_HALT,
            semaphore: Cell::new(false),
            pc: 0,
            ibist: 0,
            dma_count: 0,
            dma_bytes_total: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    #[must_use]
    pub const fn is_halted(&self) -> bool {
        (self.status & Self::STATUS_HALT) != 0
    }

    #[must_use]
    pub const fn is_single_step(&self) -> bool {
        (self.status & Self::STATUS_SSTEP) != 0
    }

    #[must_use]
    pub const fn intr_on_break(&self) -> bool {
        (self.status & Self::STATUS_INTR_ON_BREAK) != 0
    }

    /// Read an SP MMIO register (`offset = pa - 0x0404_0000`).
    pub fn read_reg(&self, offset: u32) -> Result<u32, Trap> {
        match offset {
            0x00000 => Ok(self.mem_addr & 0x1FF8),
            0x00004 => Ok(self.dram_addr & 0x00FF_FFF8),
            0x00008 | 0x0000C => Ok(self.dma_len),
            0x00010 => Ok(self.status & 0x7FFF),
            0x00014 => Ok(u32::from((self.status & Self::STATUS_DMA_FULL) != 0)),
            0x00018 => Ok(u32::from((self.status & Self::STATUS_DMA_BUSY) != 0)),
            0x0001C => {
                let prev = self.semaphore.replace(true);
                Ok(u32::from(prev))
            }
            0x40000 => Ok(u32::from(self.pc & 0x0FFC)),
            0x40004 => Ok(self.ibist & 0x7F),
            _ => Err(Trap::UnimplementedRegister {
                device: "SP",
                offset,
                is_write: false,
            }),
        }
    }

    /// Write an SP MMIO register (`offset = pa - 0x0404_0000`) and return any bus action.
    pub fn write_reg_action(&mut self, offset: u32, value: u32) -> Result<SpAction, Trap> {
        match offset {
            0x00000 => {
                self.mem_addr = value & 0x1FF8;
                Ok(SpAction::None)
            }
            0x00004 => {
                self.dram_addr = value & 0x00FF_FFF8;
                Ok(SpAction::None)
            }
            0x00008 => {
                self.dma_len = value;
                self.status |= Self::STATUS_DMA_BUSY;
                let row_len = ((value & 0xFFF) | 7) + 1;
                let rows = ((value >> 12) & 0xFF) + 1;
                let total_bytes = row_len * rows;
                self.dma_count += 1;
                self.dma_bytes_total += u64::from(total_bytes);
                Ok(SpAction::StartDmaRd {
                    mem_addr: self.mem_addr,
                    dram_addr: self.dram_addr,
                    len_reg: value,
                    delay_cycles: 0,
                })
            }
            0x0000C => {
                self.dma_len = value;
                self.status |= Self::STATUS_DMA_BUSY;
                let row_len = ((value & 0xFFF) | 7) + 1;
                let rows = ((value >> 12) & 0xFF) + 1;
                let total_bytes = row_len * rows;
                self.dma_count += 1;
                self.dma_bytes_total += u64::from(total_bytes);
                Ok(SpAction::StartDmaWr {
                    mem_addr: self.mem_addr,
                    dram_addr: self.dram_addr,
                    len_reg: value,
                    delay_cycles: 0,
                })
            }
            0x00010 => {
                // SP_STATUS 25-bit write layout (N64brew):
                // bit 0/1: Clear/Set HALT
                // bit 2:   Clear BROKE (single bit; no Set BROKE!)
                // bit 3/4: Clear/Set INTR
                // bit 5/6: Clear/Set SSTEP
                // bit 7/8: Clear/Set INTR_ON_BREAK
                // bit 9/10..23/24: Clear/Set SIG0..SIG7
                let mut action = SpAction::None;

                let apply_pair = |status: &mut u32, clear_bit: u32, set_bit: u32, flag: u32| {
                    let c = (value & (1 << clear_bit)) != 0;
                    let s = (value & (1 << set_bit)) != 0;
                    if c && !s {
                        *status &= !flag;
                    } else if s && !c {
                        *status |= flag;
                    }
                };

                apply_pair(&mut self.status, 0, 1, Self::STATUS_HALT);
                if (value & (1 << 2)) != 0 {
                    self.status &= !Self::STATUS_BROKE;
                }
                let clear_intr = (value & (1 << 3)) != 0;
                let set_intr = (value & (1 << 4)) != 0;
                if clear_intr && !set_intr {
                    action = SpAction::ClearIntr;
                } else if set_intr && !clear_intr {
                    action = SpAction::SetIntr;
                }
                apply_pair(&mut self.status, 5, 6, Self::STATUS_SSTEP);
                apply_pair(&mut self.status, 7, 8, Self::STATUS_INTR_ON_BREAK);
                for sig in 0..8u32 {
                    apply_pair(&mut self.status, 9 + sig * 2, 10 + sig * 2, 1 << (7 + sig));
                }
                Ok(action)
            }
            0x00014 | 0x00018 => Ok(SpAction::None),
            0x0001C => {
                self.semaphore.set(false);
                Ok(SpAction::None)
            }
            0x40000 => {
                self.pc = (value as u16) & 0x0FFC;
                Ok(SpAction::None)
            }
            0x40004 => {
                if (value & 4) != 0 {
                    self.ibist = 0;
                } else {
                    self.ibist = value & 0x03;
                }
                Ok(SpAction::None)
            }
            _ => Err(Trap::UnimplementedRegister {
                device: "SP",
                offset,
                is_write: true,
            }),
        }
    }

    /// Mark an in-flight SP DMA as completed (`DMA_BUSY` cleared).
    pub fn complete_dma(&mut self) {
        self.status &= !(Self::STATUS_DMA_BUSY | Self::STATUS_DMA_FULL);
    }

    /// Execute a 2D RDRAM -> DMEM/IMEM SP DMA transfer and update `mem_addr`/`dram_addr`/`dma_len`.
    pub fn execute_dma_rd(
        &mut self,
        rdram: &Rdram,
        dmem: &mut [u8],
        imem: &mut [u8],
        mem_addr: u32,
        dram_addr: u32,
        len_reg: u32,
    ) {
        let is_imem = (mem_addr & 0x1000) != 0;
        let mut sp_off = (mem_addr & 0x0FF8) as usize;
        let mut dram = dram_addr & 0x00FF_FFF8;
        let row_len = (((len_reg & 0xFFF) | 7) + 1) as usize;
        let rows = (((len_reg >> 12) & 0xFF) + 1) as usize;
        let skip = (len_reg >> 20) & 0x0FF8;

        let mem_bank = if is_imem { imem } else { dmem };

        for row in 0..rows {
            for _ in 0..row_len {
                let byte = rdram.read_u8(dram);
                mem_bank[sp_off & 0x0FFF] = byte;
                sp_off = (sp_off + 1) & 0x0FFF;
                dram = dram.wrapping_add(1);
            }
            if row + 1 < rows {
                dram = dram.wrapping_add(skip);
            }
        }

        self.mem_addr = (mem_addr & 0x1000) | ((sp_off as u32) & 0x0FF8);
        self.dram_addr = dram & 0x00FF_FFF8;
        self.dma_len = (len_reg & 0xFFF0_0000) | 0x0000_0FF8;
    }

    /// Execute a 2D DMEM/IMEM -> RDRAM SP DMA transfer and update `mem_addr`/`dram_addr`/`dma_len`.
    pub fn execute_dma_wr(
        &mut self,
        rdram: &mut Rdram,
        dmem: &[u8],
        imem: &[u8],
        mem_addr: u32,
        dram_addr: u32,
        len_reg: u32,
    ) {
        let is_imem = (mem_addr & 0x1000) != 0;
        let mut sp_off = (mem_addr & 0x0FF8) as usize;
        let mut dram = dram_addr & 0x00FF_FFF8;
        let row_len = (((len_reg & 0xFFF) | 7) + 1) as usize;
        let rows = (((len_reg >> 12) & 0xFF) + 1) as usize;
        let skip = (len_reg >> 20) & 0x0FF8;

        let mem_bank = if is_imem { imem } else { dmem };

        for row in 0..rows {
            for _ in 0..row_len {
                let byte = mem_bank[sp_off & 0x0FFF];
                rdram.write_u8(dram, byte);
                sp_off = (sp_off + 1) & 0x0FFF;
                dram = dram.wrapping_add(1);
            }
            if row + 1 < rows {
                dram = dram.wrapping_add(skip);
            }
        }

        self.mem_addr = (mem_addr & 0x1000) | ((sp_off as u32) & 0x0FF8);
        self.dram_addr = dram & 0x00FF_FFF8;
        self.dma_len = (len_reg & 0xFFF0_0000) | 0x0000_0FF8;
    }
}

impl Default for Sp {
    fn default() -> Self {
        Self::new()
    }
}
