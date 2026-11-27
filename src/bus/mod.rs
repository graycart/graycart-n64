//! Physical memory bus and RCP address decoder (sole memory authority).
//!
//! Cited: N64brew — Memory map & SysAD access quirks
//!   https://n64brew.dev/wiki/Memory_map · docs/02-bus-memory-dma.md
//! Cross-check: Graycart N64 Development Plan §3.5 (9th-bit RDRAM) & §5.1 (Trap, don't stub)

pub mod isviewer;
pub mod rdram;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::ai::{Ai, AiAction};
use crate::cart::Cart;
use crate::mi::{Mi, MI_INTR_AI, MI_INTR_DP, MI_INTR_PI, MI_INTR_SI, MI_INTR_SP, MI_INTR_VI};
use crate::pi::{Pi, PiAction};
use crate::pif::Pif;
use crate::rdp::{CpuRdpLle, RdpAction};
use crate::ri::Ri;
use crate::rsp::{Sp, SpAction, DMEM_SIZE, IMEM_SIZE};
use crate::schedule::EventKind;
use crate::si::{Si, SiAction};
use crate::trap::Trap;
use crate::vi::{Vi, ViAction};

pub use isviewer::{IsViewer, ISVIEWER_BASE, ISVIEWER_END, ISVIEWER_MAGIC};
pub use rdram::Rdram;

/// CPU cacheability attribute passed from MMU/TLB across SysAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheAttr {
    /// Cached access — legal ONLY inside [`PhysRegion::Rdram`].
    Cached,
    /// Uncached access — legal for RDRAM and mapped MMIO windows.
    Uncached,
}

/// Complete N64 physical address map classification (`docs/02-bus-memory-dma.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysRegion {
    /// `0x0000_0000..=0x03EF_FFFF` — RDRAM (only region permitting cached access).
    Rdram,
    /// `0x03F0_0000..=0x03FF_FFFF` — RDRAM registers (+ broadcast window).
    RdramRegs,
    /// `0x0400_0000..=0x0400_0FFF` (and 8 KiB mirrors up to `0x0403_FFFF`) — RSP DMEM.
    RspDmem,
    /// `0x0400_1000..=0x0400_1FFF` (and 8 KiB mirrors up to `0x0403_FFFF`) — RSP IMEM.
    RspImem,
    /// `0x0404_0000..=0x040B_FFFF` — RSP status/DMA/PC registers.
    RspRegs,
    /// `0x0410_0000..=0x041F_FFFF` — RDP command registers.
    RdpCmdRegs,
    /// `0x0420_0000..=0x042F_FFFF` — RDP span registers.
    RdpSpanRegs,
    /// `0x0430_0000..=0x043F_FFFF` — MIPS Interface (MI).
    MiRegs,
    /// `0x0440_0000..=0x044F_FFFF` — Video Interface (VI).
    ViRegs,
    /// `0x0450_0000..=0x045F_FFFF` — Audio Interface (AI).
    AiRegs,
    /// `0x0460_0000..=0x046F_FFFF` — Peripheral Interface (PI).
    PiRegs,
    /// `0x0470_0000..=0x047F_FFFF` — RDRAM Interface (RI).
    RiRegs,
    /// `0x0480_0000..=0x048F_FFFF` — Serial Interface (SI).
    SiRegs,
    /// `0x0500_0000..=0x05FF_FFFF` — N64DD registers (PI Domain 1).
    DdRegs,
    /// `0x0600_0000..=0x07FF_FFFF` — N64DD IPL ROM.
    DdIplRom,
    /// `0x0800_0000..=0x0FFF_FFFF` — Cartridge SRAM / FlashRAM (PI Domain 2).
    CartSramFlash,
    /// `0x13FF_0000..=0x13FF_FFFF` — ISViewer 64 debug console (when enabled).
    IsViewer,
    /// `0x1000_0000..=0x1FBF_FFFF` — Cartridge ROM (PI Domain 1).
    CartRom,
    /// `0x1FC0_0000..=0x1FC0_07BF` — PIF ROM (boot).
    PifRom,
    /// `0x1FC0_07C0..=0x1FC0_07FF` — PIF RAM (64 bytes).
    PifRam,
    /// Unmapped physical windows that freeze retail N64 hardware:
    /// `0x040C_0000..=0x040F_FFFF`, `0x0490_0000..=0x04FF_FFFF`,
    /// `0x1FC0_0800..=0x7FFF_FFFF`, `0x8000_0000..=0xFFFF_FFFF`.
    UnmappedFreeze,
}

/// Classify a 32-bit physical address according to the RCP decode table.
#[must_use]
pub fn decode_physical(pa: u32, isviewer_enabled: bool) -> PhysRegion {
    match pa {
        0x0000_0000..=0x03EF_FFFF => PhysRegion::Rdram,
        0x03F0_0000..=0x03FF_FFFF => PhysRegion::RdramRegs,
        // RSP DMEM/IMEM mirror across 0x0400_0000..=0x0403_FFFF in 8 KiB strides.
        0x0400_0000..=0x0403_FFFF => {
            if (pa & 0x1000) == 0 {
                PhysRegion::RspDmem
            } else {
                PhysRegion::RspImem
            }
        }
        0x0404_0000..=0x040B_FFFF => PhysRegion::RspRegs,
        0x040C_0000..=0x040F_FFFF => PhysRegion::UnmappedFreeze,
        0x0410_0000..=0x041F_FFFF => PhysRegion::RdpCmdRegs,
        0x0420_0000..=0x042F_FFFF => PhysRegion::RdpSpanRegs,
        0x0430_0000..=0x043F_FFFF => PhysRegion::MiRegs,
        0x0440_0000..=0x044F_FFFF => PhysRegion::ViRegs,
        0x0450_0000..=0x045F_FFFF => PhysRegion::AiRegs,
        0x0460_0000..=0x046F_FFFF => PhysRegion::PiRegs,
        0x0470_0000..=0x047F_FFFF => PhysRegion::RiRegs,
        0x0480_0000..=0x048F_FFFF => PhysRegion::SiRegs,
        0x0490_0000..=0x04FF_FFFF => PhysRegion::UnmappedFreeze,
        0x0500_0000..=0x05FF_FFFF => PhysRegion::DdRegs,
        0x0600_0000..=0x07FF_FFFF => PhysRegion::DdIplRom,
        0x0800_0000..=0x0FFF_FFFF => PhysRegion::CartSramFlash,
        ISVIEWER_BASE..=ISVIEWER_END if isviewer_enabled => PhysRegion::IsViewer,
        0x1000_0000..=0x1FBF_FFFF => PhysRegion::CartRom,
        0x1FC0_0000..=0x1FC0_07BF => PhysRegion::PifRom,
        0x1FC0_07C0..=0x1FC0_07FF => PhysRegion::PifRam,
        _ => PhysRegion::UnmappedFreeze,
    }
}

/// Returns `true` if `pa` lies within the RCP internal register/memory window
/// (`0x0400_0000..=0x04FF_FFFF`) where 64-bit reads freeze hardware.
#[must_use]
pub const fn is_rcp_window(pa: u32) -> bool {
    pa >= 0x0400_0000 && pa <= 0x04FF_FFFF
}

/// Physical bus owner — sole authority over all memory and MMIO routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bus {
    pub rdram: Rdram,
    /// RSP DMEM (`0x0400_0000..=0x0400_0FFF`, single source of truth for CPU & RSP).
    pub sp_dmem: Vec<u8>,
    /// RSP IMEM (`0x0400_1000..=0x0400_1FFF`, single source of truth for CPU & RSP).
    pub sp_imem: Vec<u8>,
    /// RSP (SP) MMIO registers and 2D DMA state (`0x0404_0000..=0x040B_FFFF`).
    pub sp: Sp,
    /// RDP DPC/DPS MMIO (`0x0410_0000` / `0x0420_0000`). Single source of truth.
    pub rdp: CpuRdpLle,
    pub mi: Mi,
    pub vi: Vi,
    pub ai: Ai,
    pub pi: Pi,
    pub ri: Ri,
    pub si: Si,
    pub pif: Pif,
    pub isviewer: IsViewer,
    pub cart: Option<Cart>,
    pub isviewer_enabled: bool,
    /// Pending PI DMA byte count for `complete_pi_dma`.
    pub pending_pi_len: u32,
    /// Out-of-band scheduler events queued by MMIO DMA triggers (drained by `N64Core`).
    #[serde(skip)]
    pub pending_events: Vec<(u64, EventKind)>,
}

impl Bus {
    #[must_use]
    pub fn new(expansion_pak: bool, isviewer_enabled: bool) -> Self {
        Self {
            rdram: Rdram::new(expansion_pak),
            sp_dmem: vec![0; DMEM_SIZE],
            sp_imem: vec![0; IMEM_SIZE],
            sp: Sp::new(),
            rdp: CpuRdpLle::new(),
            mi: Mi::new(),
            vi: Vi::new(),
            ai: Ai::new(),
            pi: Pi::new(),
            ri: Ri::new(),
            si: Si::new(),
            pif: Pif::new(),
            isviewer: IsViewer::new(),
            cart: None,
            isviewer_enabled,
            pending_pi_len: 0,
            pending_events: Vec::new(),
        }
    }

    /// Drain queued peripheral scheduler requests into the master scheduler.
    pub fn drain_pending_events(&mut self) -> Vec<(u64, EventKind)> {
        std::mem::take(&mut self.pending_events)
    }

    /// Complete an in-flight PI DMA (clears `PI_STATUS.DMA_BUSY`, asserts `MI_INTR_PI`).
    pub fn complete_pi_dma(&mut self) {
        let len = self.pending_pi_len;
        self.pending_pi_len = 0;
        self.pi.complete_dma(len);
        self.mi.set_intr(MI_INTR_PI);
    }

    /// Complete an in-flight SI DMA (clears `SI_STATUS.DMA_BUSY`, asserts `MI_INTR_SI`).
    pub fn complete_si_dma(&mut self) {
        self.si.complete_dma();
        self.mi.set_intr(MI_INTR_SI);
    }

    /// Complete an in-flight SP DMA (clears `SP_STATUS.DMA_BUSY`).
    pub fn complete_sp_dma(&mut self) {
        self.sp.complete_dma();
    }

    /// Execute immediate-data Cart → RDRAM PI DMA copy (interrupt delayed per §3.3).
    fn copy_cart_to_rdram(&mut self, dram_addr: u32, cart_addr: u32, len: u32) {
        let rom = self.cart.as_ref().map(Cart::rom).unwrap_or(&[]);
        for i in 0..len {
            let ca = cart_addr.wrapping_add(i);
            let byte = if ca >= 0x1000_0000 {
                let off = (ca - 0x1000_0000) as usize;
                rom.get(off).copied().unwrap_or(0)
            } else {
                0
            };
            self.rdram.write_u8(dram_addr.wrapping_add(i), byte);
        }
    }

    /// Copy 64 bytes from PIF RAM into RDRAM at `dram_addr` (running Joybus HLE first).
    fn copy_pif_to_rdram(&mut self, dram_addr: u32) {
        self.pif.execute_joybus();
        for (i, &b) in self.pif.ram.iter().enumerate() {
            self.rdram.write_u8(dram_addr.wrapping_add(i as u32), b);
        }
    }

    /// Copy 64 bytes from RDRAM at `dram_addr` into PIF RAM and run PIF write commands.
    fn copy_rdram_to_pif(&mut self, dram_addr: u32) {
        for i in 0..self.pif.ram.len() {
            self.pif.ram[i] = self.rdram.read_u8(dram_addr.wrapping_add(i as u32));
        }
        self.pif.process_write_command();
    }

    pub fn attach_cart(&mut self, cart: Cart) {
        self.cart = Some(cart);
    }

    /// Copy up to 1 MiB from cart offset `0x1000` into RDRAM at `entry_pc & 0x1FFF_FFFF`
    /// (matching IPL3's 1 MiB program load destination for soft-boot test suites).
    pub fn copy_cart_soft_boot(&mut self, entry_pc: u32) {
        let Some(cart) = &self.cart else {
            return;
        };
        let rom = cart.rom();
        if rom.len() <= 0x1000 {
            return;
        }
        let dst_off = (entry_pc & 0x1FFF_FFFF) as usize;
        if dst_off >= self.rdram.len() {
            return;
        }
        let available = (rom.len() - 0x1000).min(0x0010_0000);
        let dst_cap = self.rdram.len().saturating_sub(dst_off);
        let copy_len = available.min(dst_cap);
        self.rdram.as_mut_slice()[dst_off..dst_off + copy_len]
            .copy_from_slice(&rom[0x1000..0x1000 + copy_len]);
    }

    #[must_use]
    pub fn isviewer_output(&self) -> &[u8] {
        self.isviewer.output()
    }

    fn check_access(
        &self,
        pa: u32,
        size: u8,
        is_write: bool,
        cache: CacheAttr,
    ) -> Result<PhysRegion, Trap> {
        let region = decode_physical(pa, self.isviewer_enabled);
        if cache == CacheAttr::Cached && region != PhysRegion::Rdram {
            return Err(Trap::CachedMmioFreeze { pa });
        }
        if region == PhysRegion::UnmappedFreeze {
            return Err(Trap::UnmappedFreeze { pa, size, is_write });
        }
        if !is_write && size == 8 && is_rcp_window(pa) {
            return Err(Trap::Rcp64BitReadFreeze { pa });
        }
        Ok(region)
    }

    /// Read a byte at physical address `pa` with SysAD cache attribute enforcement.
    pub fn read_u8(&self, pa: u32, cache: CacheAttr) -> Result<u8, Trap> {
        let region = self.check_access(pa, 1, false, cache)?;
        match region {
            PhysRegion::Rdram => Ok(self.rdram.read_u8(pa)),
            PhysRegion::RspDmem => Ok(self.sp_dmem[(pa & 0x0FFF) as usize]),
            PhysRegion::RspImem => Ok(self.sp_imem[(pa & 0x0FFF) as usize]),
            PhysRegion::IsViewer => Ok(self.isviewer.read_u8(pa - ISVIEWER_BASE)),
            PhysRegion::CartRom => {
                let offset = (pa - 0x1000_0000) as usize;
                Ok(self
                    .cart
                    .as_ref()
                    .and_then(|c| c.rom().get(offset).copied())
                    .unwrap_or(0))
            }
            PhysRegion::PifRam => Ok(self.pif.ram[(pa - 0x1FC0_07C0) as usize]),
            other => Err(self.unimplemented_region_trap(other, pa, false)),
        }
    }

    /// Write a byte at physical address `pa` with SysAD cache attribute enforcement.
    pub fn write_u8(&mut self, pa: u32, val: u8, cache: CacheAttr) -> Result<(), Trap> {
        let region = self.check_access(pa, 1, true, cache)?;
        match region {
            PhysRegion::Rdram => {
                self.rdram.write_u8(pa, val);
                Ok(())
            }
            PhysRegion::RspDmem => {
                self.sp_dmem[(pa & 0x0FFF) as usize] = val;
                Ok(())
            }
            PhysRegion::RspImem => {
                self.sp_imem[(pa & 0x0FFF) as usize] = val;
                Ok(())
            }
            PhysRegion::IsViewer => {
                self.isviewer.write_u8(pa - ISVIEWER_BASE, val);
                Ok(())
            }
            PhysRegion::PifRam => {
                self.pif.ram[(pa - 0x1FC0_07C0) as usize] = val;
                Ok(())
            }
            other => Err(self.unimplemented_region_trap(other, pa, true)),
        }
    }

    /// Read a big-endian 16-bit halfword at physical address `pa`.
    pub fn read_u16(&self, pa: u32, cache: CacheAttr) -> Result<u16, Trap> {
        if pa & 1 != 0 {
            return Err(Trap::MisalignedAccess { pa, size: 2 });
        }
        let region = self.check_access(pa, 2, false, cache)?;
        match region {
            PhysRegion::Rdram => self.rdram.read_u16(pa),
            PhysRegion::RspDmem => {
                let idx = (pa & 0x0FFF) as usize;
                Ok(u16::from_be_bytes([
                    self.sp_dmem[idx],
                    self.sp_dmem[idx + 1],
                ]))
            }
            PhysRegion::RspImem => {
                let idx = (pa & 0x0FFF) as usize;
                Ok(u16::from_be_bytes([
                    self.sp_imem[idx],
                    self.sp_imem[idx + 1],
                ]))
            }
            PhysRegion::IsViewer => {
                let off = pa - ISVIEWER_BASE;
                Ok(u16::from_be_bytes([
                    self.isviewer.read_u8(off),
                    self.isviewer.read_u8(off + 1),
                ]))
            }
            PhysRegion::CartRom => {
                let offset = (pa - 0x1000_0000) as usize;
                let rom = self.cart.as_ref().map(Cart::rom).unwrap_or(&[]);
                if offset + 2 <= rom.len() {
                    Ok(u16::from_be_bytes([rom[offset], rom[offset + 1]]))
                } else {
                    Ok(0)
                }
            }
            PhysRegion::PifRam => {
                let idx = (pa - 0x1FC0_07C0) as usize;
                Ok(u16::from_be_bytes([
                    self.pif.ram[idx],
                    self.pif.ram[idx + 1],
                ]))
            }
            other => Err(self.unimplemented_region_trap(other, pa, false)),
        }
    }

    /// Write a big-endian 16-bit halfword at physical address `pa`.
    pub fn write_u16(&mut self, pa: u32, val: u16, cache: CacheAttr) -> Result<(), Trap> {
        if pa & 1 != 0 {
            return Err(Trap::MisalignedAccess { pa, size: 2 });
        }
        let region = self.check_access(pa, 2, true, cache)?;
        match region {
            PhysRegion::Rdram => self.rdram.write_u16(pa, val),
            PhysRegion::RspDmem => {
                let idx = (pa & 0x0FFF) as usize;
                self.sp_dmem[idx..idx + 2].copy_from_slice(&val.to_be_bytes());
                Ok(())
            }
            PhysRegion::RspImem => {
                let idx = (pa & 0x0FFF) as usize;
                self.sp_imem[idx..idx + 2].copy_from_slice(&val.to_be_bytes());
                Ok(())
            }
            PhysRegion::IsViewer => {
                let off = pa - ISVIEWER_BASE;
                let bytes = val.to_be_bytes();
                self.isviewer.write_u8(off, bytes[0]);
                self.isviewer.write_u8(off + 1, bytes[1]);
                Ok(())
            }
            PhysRegion::PifRam => {
                let idx = (pa - 0x1FC0_07C0) as usize;
                self.pif.ram[idx..idx + 2].copy_from_slice(&val.to_be_bytes());
                Ok(())
            }
            other => Err(self.unimplemented_region_trap(other, pa, true)),
        }
    }

    /// Read a big-endian 32-bit word at physical address `pa`.
    pub fn read_u32(&self, pa: u32, cache: CacheAttr) -> Result<u32, Trap> {
        if pa & 3 != 0 {
            return Err(Trap::MisalignedAccess { pa, size: 4 });
        }
        let region = self.check_access(pa, 4, false, cache)?;
        match region {
            PhysRegion::Rdram => self.rdram.read_u32(pa),
            PhysRegion::RspDmem => {
                let idx = (pa & 0x0FFF) as usize;
                Ok(u32::from_be_bytes([
                    self.sp_dmem[idx],
                    self.sp_dmem[idx + 1],
                    self.sp_dmem[idx + 2],
                    self.sp_dmem[idx + 3],
                ]))
            }
            PhysRegion::RspImem => {
                let idx = (pa & 0x0FFF) as usize;
                Ok(u32::from_be_bytes([
                    self.sp_imem[idx],
                    self.sp_imem[idx + 1],
                    self.sp_imem[idx + 2],
                    self.sp_imem[idx + 3],
                ]))
            }
            PhysRegion::RspRegs => self.sp.read_reg(pa - 0x0404_0000),
            PhysRegion::MiRegs => self.mi.read_reg(pa & 0xFFFF),
            PhysRegion::ViRegs => self.vi.read_reg(pa & 0xFFFF),
            PhysRegion::AiRegs => self.ai.read_reg(pa & 0xFFFF),
            PhysRegion::PiRegs => self.pi.read_reg(pa & 0xFFFF),
            PhysRegion::RiRegs => self.ri.read_reg(pa & 0xFFFF),
            PhysRegion::SiRegs => self.si.read_reg(pa & 0xFFFF),
            PhysRegion::RdpCmdRegs => self.rdp.read_cmd_reg(pa & 0xFFFF),
            PhysRegion::RdpSpanRegs => self.rdp.read_span_reg(pa & 0xFFFF),
            PhysRegion::IsViewer => Ok(self.isviewer.read_u32(pa - ISVIEWER_BASE)),
            PhysRegion::CartRom => {
                let offset = (pa - 0x1000_0000) as usize;
                let rom = self.cart.as_ref().map(Cart::rom).unwrap_or(&[]);
                if offset + 4 <= rom.len() {
                    Ok(u32::from_be_bytes([
                        rom[offset],
                        rom[offset + 1],
                        rom[offset + 2],
                        rom[offset + 3],
                    ]))
                } else {
                    Ok(0)
                }
            }
            PhysRegion::PifRam => {
                let idx = (pa - 0x1FC0_07C0) as usize;
                Ok(u32::from_be_bytes([
                    self.pif.ram[idx],
                    self.pif.ram[idx + 1],
                    self.pif.ram[idx + 2],
                    self.pif.ram[idx + 3],
                ]))
            }
            other => Err(self.unimplemented_region_trap(other, pa, false)),
        }
    }

    /// Write a big-endian 32-bit word at physical address `pa`.
    pub fn write_u32(&mut self, pa: u32, val: u32, cache: CacheAttr) -> Result<(), Trap> {
        if pa & 3 != 0 {
            return Err(Trap::MisalignedAccess { pa, size: 4 });
        }
        let region = self.check_access(pa, 4, true, cache)?;
        match region {
            PhysRegion::Rdram => self.rdram.write_u32(pa, val),
            PhysRegion::RspDmem => {
                let idx = (pa & 0x0FFF) as usize;
                self.sp_dmem[idx..idx + 4].copy_from_slice(&val.to_be_bytes());
                Ok(())
            }
            PhysRegion::RspImem => {
                let idx = (pa & 0x0FFF) as usize;
                self.sp_imem[idx..idx + 4].copy_from_slice(&val.to_be_bytes());
                Ok(())
            }
            PhysRegion::RspRegs => {
                match self.sp.write_reg_action(pa - 0x0404_0000, val)? {
                    SpAction::StartDmaRd {
                        mem_addr,
                        dram_addr,
                        len_reg,
                        delay_cycles,
                    } => {
                        self.sp.execute_dma_rd(
                            &self.rdram,
                            &mut self.sp_dmem,
                            &mut self.sp_imem,
                            mem_addr,
                            dram_addr,
                            len_reg,
                        );
                        if delay_cycles == 0 {
                            self.sp.complete_dma();
                        } else {
                            self.pending_events
                                .push((delay_cycles, EventKind::SpDmaComplete));
                        }
                    }
                    SpAction::StartDmaWr {
                        mem_addr,
                        dram_addr,
                        len_reg,
                        delay_cycles,
                    } => {
                        self.sp.execute_dma_wr(
                            &mut self.rdram,
                            &self.sp_dmem,
                            &self.sp_imem,
                            mem_addr,
                            dram_addr,
                            len_reg,
                        );
                        if delay_cycles == 0 {
                            self.sp.complete_dma();
                        } else {
                            self.pending_events
                                .push((delay_cycles, EventKind::SpDmaComplete));
                        }
                    }
                    SpAction::ClearIntr => self.mi.clear_intr(MI_INTR_SP),
                    SpAction::SetIntr => self.mi.set_intr(MI_INTR_SP),
                    SpAction::None => {}
                }
                Ok(())
            }
            PhysRegion::MiRegs => self.mi.write_reg(pa & 0xFFFF, val),
            PhysRegion::ViRegs => {
                if let ViAction::ClearIntr = self.vi.write_reg_action(pa & 0xFFFF, val)? {
                    self.mi.clear_intr(MI_INTR_VI);
                }
                Ok(())
            }
            PhysRegion::AiRegs => {
                match self.ai.write_reg_action(pa & 0xFFFF, val)? {
                    AiAction::StartDma { delay_cycles } => {
                        self.pending_events
                            .push((delay_cycles, EventKind::AiDmaStep));
                    }
                    AiAction::ClearIntr => self.mi.clear_intr(MI_INTR_AI),
                    AiAction::None => {}
                }
                Ok(())
            }
            PhysRegion::PiRegs => {
                match self.pi.write_reg_action(pa & 0xFFFF, val)? {
                    PiAction::StartCartToRdram {
                        dram_addr,
                        cart_addr,
                        len,
                        delay_cycles,
                    } => {
                        // Immediate-data / delayed-MI-interrupt PI DMA (§3.3).
                        self.copy_cart_to_rdram(dram_addr, cart_addr, len);
                        self.pending_pi_len = len;
                        self.pending_events
                            .push((delay_cycles, EventKind::PiDmaComplete));
                    }
                    PiAction::StartRdramToCart {
                        len, delay_cycles, ..
                    } => {
                        self.pending_pi_len = len;
                        self.pending_events
                            .push((delay_cycles, EventKind::PiDmaComplete));
                    }
                    PiAction::ClearIntr => self.mi.clear_intr(MI_INTR_PI),
                    PiAction::Reset => {
                        self.pending_pi_len = 0;
                    }
                    PiAction::None => {}
                }
                Ok(())
            }
            PhysRegion::RiRegs => self.ri.write_reg(pa & 0xFFFF, val),
            PhysRegion::SiRegs => {
                match self.si.write_reg_action(pa & 0xFFFF, val)? {
                    SiAction::DmaPifToRdram {
                        dram_addr,
                        delay_cycles,
                    } => {
                        self.copy_pif_to_rdram(dram_addr);
                        self.pending_events
                            .push((delay_cycles, EventKind::SiDmaComplete));
                    }
                    SiAction::DmaRdramToPif {
                        dram_addr,
                        delay_cycles,
                    } => {
                        self.copy_rdram_to_pif(dram_addr);
                        self.pending_events
                            .push((delay_cycles, EventKind::SiDmaComplete));
                    }
                    SiAction::ClearIntr => self.mi.clear_intr(MI_INTR_SI),
                    SiAction::None => {}
                }
                Ok(())
            }
            PhysRegion::RdpCmdRegs => {
                match self.rdp.write_cmd_reg_action(pa & 0xFFFF, val)? {
                    RdpAction::RaiseDpIntr => self.mi.set_intr(MI_INTR_DP),
                    RdpAction::None => {}
                }
                Ok(())
            }
            PhysRegion::RdpSpanRegs => {
                match self.rdp.write_span_reg_action(pa & 0xFFFF, val)? {
                    RdpAction::RaiseDpIntr => self.mi.set_intr(MI_INTR_DP),
                    RdpAction::None => {}
                }
                Ok(())
            }
            PhysRegion::IsViewer => {
                self.isviewer.write_u32(pa - ISVIEWER_BASE, val);
                Ok(())
            }
            PhysRegion::PifRam => {
                let idx = (pa - 0x1FC0_07C0) as usize;
                self.pif.ram[idx..idx + 4].copy_from_slice(&val.to_be_bytes());
                if idx == 60 {
                    self.pif.process_write_command();
                }
                Ok(())
            }
            other => Err(self.unimplemented_region_trap(other, pa, true)),
        }
    }

    /// Read a big-endian 64-bit doubleword at physical address `pa`.
    ///
    /// 64-bit reads in the RCP window (`0x0400_0000..=0x04FF_FFFF`) trap with
    /// [`Trap::Rcp64BitReadFreeze`] matching retail SysAD hardware behavior.
    pub fn read_u64(&self, pa: u32, cache: CacheAttr) -> Result<u64, Trap> {
        if pa & 7 != 0 {
            return Err(Trap::MisalignedAccess { pa, size: 8 });
        }
        let region = self.check_access(pa, 8, false, cache)?;
        match region {
            PhysRegion::Rdram => self.rdram.read_u64(pa),
            other => Err(self.unimplemented_region_trap(other, pa, false)),
        }
    }

    /// Write a big-endian 64-bit doubleword at physical address `pa`.
    pub fn write_u64(&mut self, pa: u32, val: u64, cache: CacheAttr) -> Result<(), Trap> {
        if pa & 7 != 0 {
            return Err(Trap::MisalignedAccess { pa, size: 8 });
        }
        let region = self.check_access(pa, 8, true, cache)?;
        match region {
            PhysRegion::Rdram => self.rdram.write_u64(pa, val),
            other => Err(self.unimplemented_region_trap(other, pa, true)),
        }
    }

    fn unimplemented_region_trap(&self, region: PhysRegion, pa: u32, is_write: bool) -> Trap {
        let device = match region {
            PhysRegion::RdramRegs => "RDRAM_REGS",
            PhysRegion::RspRegs => "SP",
            PhysRegion::RdpCmdRegs => "DP_CMD",
            PhysRegion::RdpSpanRegs => "DP_SPAN",
            PhysRegion::MiRegs => "MI",
            PhysRegion::ViRegs => "VI",
            PhysRegion::AiRegs => "AI",
            PhysRegion::PiRegs => "PI",
            PhysRegion::RiRegs => "RI",
            PhysRegion::SiRegs => "SI",
            PhysRegion::DdRegs => "N64DD_REGS",
            PhysRegion::DdIplRom => "N64DD_IPL",
            PhysRegion::CartSramFlash => "CART_SAVE",
            PhysRegion::CartRom => "CART_ROM",
            PhysRegion::PifRom => "PIF_ROM",
            _ => "UNKNOWN",
        };
        Trap::UnimplementedRegister {
            device,
            offset: pa & 0xFFFF,
            is_write,
        }
    }
}
