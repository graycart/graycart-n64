//! Graycart Nintendo 64 library-first emulator core.
//!
//! Architecture contract:
//! - Deterministic, library-first core with zero GUI dependencies.
//! - Bus is the sole physical memory authority (including the RDRAM 9th-bit side array).
//! - Backend traits separate CPU, RSP, and RDP (`CpuRdpLle` oracle + future `EdgeWalker`
//!   bespoke Vulkan compute LLE RDP).
//! - Tier 1 (`N64Config::accurate()`) is strictly hardware-faithful; Tier 2
//!   (`EnhancementConfig`) is explicitly opt-in and never enabled in accuracy/CI gates.
//!
//! Cited: Graycart N64 architecture & implementation plan
//!   docs/00-architecture-overview.md · docs/08-implementation-plan.md · docs/DEVELOPMENT-PLAN.md

pub mod ai;
pub mod bus;
pub mod cart;
pub mod cpu;
pub mod mi;
pub mod pi;
pub mod pif;
pub mod rcp;
pub mod rdp;
pub mod ri;
pub mod rsp;
pub mod schedule;
pub mod si;
pub mod trap;
pub mod vi;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

pub use bus::{Bus, CacheAttr, IsViewer, PhysRegion, Rdram};
pub use cart::{Cart, CicType, RomError, RomFormat, RomHeader};
pub use cpu::{CpuBackend, InterpCpu};
pub use rdp::{CpuRdpLle, RdpBackend, RdpSyncKind};
pub use rsp::{InterpRsp, RspBackend};
pub use schedule::{CountClock, EventKind, ScheduledEvent, Scheduler};
pub use trap::Trap;

/// Tier 2 opt-in enhancement settings (reserved for M11).
///
/// Always disabled under [`N64Config::accurate()`] and in CI accuracy gates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnhancementConfig {
    /// Internal resolution multiplier for EdgeWalker (1 = native accurate).
    pub resolution_scale: u8,
    /// Voluntary VI-locked CPU/RCP overclock percentage (100 = stock 93.75 MHz).
    pub cpu_overclock_percent: u16,
    /// Disable VI anti-alias blur filter on scanout.
    pub vi_deblur: bool,
    /// Enable extended-frustum widescreen when supported.
    pub widescreen: bool,
}

impl Default for EnhancementConfig {
    fn default() -> Self {
        Self {
            resolution_scale: 1,
            cpu_overclock_percent: 100,
            vi_deblur: false,
            widescreen: false,
        }
    }
}

/// Machine configuration for [`N64Core`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct N64Config {
    /// Whether the 4 MiB Expansion Pak is installed (8 MiB total RDRAM).
    pub expansion_pak: bool,
    /// Enable HLE soft-boot path (skips PIF ROM / IPL3 when requested by young CPU suites).
    pub soft_boot: bool,
    /// Clamp scheduler slices to 1 instruction for deterministic debugging.
    pub single_step_mode: bool,
    /// Enable the ISViewer debug cart window (`0x13FF_0000..=0x13FF_FFFF`).
    pub isviewer_enabled: bool,
    /// Tier 2 opt-in enhancements (all stock/off in [`N64Config::accurate()`]).
    pub enhancements: EnhancementConfig,
}

impl N64Config {
    /// Strictly hardware-faithful Tier 1 configuration used by CI and accuracy gates.
    #[must_use]
    pub fn accurate() -> Self {
        Self {
            expansion_pak: true,
            soft_boot: false,
            single_step_mode: false,
            isviewer_enabled: true,
            enhancements: EnhancementConfig::default(),
        }
    }
}

impl Default for N64Config {
    fn default() -> Self {
        Self::accurate()
    }
}

/// Result of running [`N64Core`] for a cycle or frame budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunOutcome {
    /// Ran cleanly to the requested cycle or frame budget.
    Completed { cycles: u64, frames: u64 },
    /// Stopped on an explicit hardware freeze or unimplemented-path trap.
    Trapped { cycles: u64, trap: Trap },
}

/// Top-level serializable Nintendo 64 machine core.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct N64Core {
    pub config: N64Config,
    pub scheduler: Scheduler,
    pub cpu: InterpCpu,
    pub rsp: InterpRsp,
    pub rdp: CpuRdpLle,
    pub bus: Bus,
}

impl N64Core {
    /// Construct a fresh, powered-on core with no cartridge inserted.
    #[must_use]
    pub fn new(config: N64Config) -> Self {
        let mut scheduler = Scheduler::new();
        scheduler.single_step_mode = config.single_step_mode;
        let bus = Bus::new(config.expansion_pak, config.isviewer_enabled);
        Self {
            config,
            scheduler,
            cpu: InterpCpu::new(),
            rsp: InterpRsp::new(),
            rdp: CpuRdpLle::new(),
            bus,
        }
    }

    /// Construct a core with a validated, canonicalized cartridge loaded.
    #[must_use]
    pub fn with_cart(cart: Cart, config: N64Config) -> Self {
        let mut core = Self::new(config);
        core.load_cart(cart);
        core
    }

    /// Attach a cartridge and apply the configured reset/boot state.
    pub fn load_cart(&mut self, cart: Cart) {
        let entry_pc = cart.header().entry_pc;
        let cic = cart.cic();
        self.bus.attach_cart(cart);
        self.reset_boot_state(entry_pc, cic);
    }

    /// Reset machine state and seed the HLE soft-boot stub if configured.
    fn reset_boot_state(&mut self, entry_pc: u32, cic: CicType) {
        self.scheduler.reset();
        self.scheduler.single_step_mode = self.config.single_step_mode;
        self.cpu.reset();
        self.rsp.reset();
        self.bus.sp.reset();
        self.bus.pif.seed_boot_ram(cic);
        self.bus.pending_events.clear();
        self.bus.pending_pi_len = 0;

        // Start register-driven VI scanline schedule.
        self.scheduler
            .schedule_after(self.bus.vi.cycles_per_line(), EventKind::ViLine);

        if self.config.soft_boot {
            // Soft-boot stub: copy up to 1 MiB from cart offset 0x1000 into RDRAM at
            // (entry_pc & 0x1FFF_FFFF), seed standard OS low-memory boot variables
            // (osTvType=NTSC, osRomBase, osMemSize), and jump to header entry_pc.
            self.bus.copy_cart_soft_boot(entry_pc);
            let _ = self.bus.rdram.write_u32(0x0000_0300, 1); // osTvType = NTSC
            let _ = self.bus.rdram.write_u32(0x0000_0304, 0); // osRomType = Cartridge
            let _ = self.bus.rdram.write_u32(0x0000_0308, 0xB000_0000); // osRomBase
            let _ = self.bus.rdram.write_u32(0x0000_030C, 0); // osResetType = Cold
            let _ = self
                .bus
                .rdram
                .write_u32(0x0000_0310, u32::from(pif::Pif::cic_seed(cic)));
            let _ = self
                .bus
                .rdram
                .write_u32(0x0000_0318, self.bus.rdram.len() as u32); // osMemSize
            self.cpu.set_pc(entry_pc as i32 as i64 as u64);
        } else {
            // Cold reset vector in KSEG1 PIF ROM space (0xBFC0_0000).
            self.cpu.set_pc(0xFFFF_FFFF_BFC0_0000);
        }
    }

    /// Execute up to `max_cycles` CPU cycles, stopping early on any [`Trap`].
    pub fn run_cycles(&mut self, max_cycles: u64) -> RunOutcome {
        let target = self.scheduler.now_cpu_cycles().saturating_add(max_cycles);
        let start = self.scheduler.now_cpu_cycles();
        while self.scheduler.now_cpu_cycles() < target {
            self.cpu.cop0.set_mi_interrupt(self.bus.mi.irq_pending());
            match self.cpu.step(&mut self.bus, &mut self.scheduler) {
                Ok(elapsed) => {
                    let step = elapsed.max(1);
                    for (delay, kind) in self.bus.drain_pending_events() {
                        self.scheduler.schedule_after(delay, kind);
                    }
                    let rcp_cycles = self.scheduler.advance(step);
                    if rcp_cycles > 0 && !self.bus.sp.is_halted() {
                        if let Err(trap) = self.rsp.run(&mut self.bus, rcp_cycles) {
                            return RunOutcome::Trapped {
                                cycles: self.scheduler.now_cpu_cycles().saturating_sub(start),
                                trap,
                            };
                        }
                        for (delay, kind) in self.bus.drain_pending_events() {
                            self.scheduler.schedule_after(delay, kind);
                        }
                    }
                    while let Some(ev) = self.scheduler.pop_due() {
                        match ev.kind {
                            EventKind::CompareMatch => {
                                self.cpu
                                    .cop0
                                    .count_clock
                                    .on_compare_event(&mut self.scheduler);
                                self.cpu.cop0.sync_timer_ip7();
                            }
                            EventKind::PiDmaComplete => {
                                self.bus.complete_pi_dma();
                            }
                            EventKind::SiDmaComplete => {
                                self.bus.complete_si_dma();
                            }
                            EventKind::ViLine => {
                                let (vi_intr, _frame_done) = self.bus.vi.step_line();
                                if vi_intr {
                                    self.bus.mi.set_intr(mi::MI_INTR_VI);
                                }
                                let next_line = self.bus.vi.cycles_per_line();
                                self.scheduler.schedule_after(next_line, EventKind::ViLine);
                            }
                            EventKind::AiDmaStep => {
                                self.bus.mi.set_intr(mi::MI_INTR_AI);
                                if let Some(next_delay) = self.bus.ai.on_dma_complete() {
                                    self.scheduler
                                        .schedule_after(next_delay, EventKind::AiDmaStep);
                                }
                            }
                            EventKind::SpDmaComplete => {
                                self.bus.complete_sp_dma();
                            }
                        }
                    }
                    self.cpu.cop0.set_mi_interrupt(self.bus.mi.irq_pending());
                }
                Err(trap) => {
                    return RunOutcome::Trapped {
                        cycles: self.scheduler.now_cpu_cycles().saturating_sub(start),
                        trap,
                    };
                }
            }
        }
        RunOutcome::Completed {
            cycles: self.scheduler.now_cpu_cycles().saturating_sub(start),
            frames: 0,
        }
    }

    /// Execute up to `frames` VI fields (coarse M0 cycle budget until VI timing lands in M2/M5).
    pub fn run_frames(&mut self, frames: u64) -> RunOutcome {
        // Approximate NTSC field cycles (93.75 MHz / 60 Hz ≈ 1_562_500 CPU cycles).
        const CYCLES_PER_FIELD_APPROX: u64 = 1_562_500;
        let budget = frames.saturating_mul(CYCLES_PER_FIELD_APPROX);
        match self.run_cycles(budget) {
            RunOutcome::Completed { cycles, .. } => RunOutcome::Completed { cycles, frames },
            trapped @ RunOutcome::Trapped { .. } => trapped,
        }
    }

    /// Return all UTF-8 / raw bytes captured via the ISViewer debug window so far.
    #[must_use]
    pub fn isviewer_output(&self) -> &[u8] {
        self.bus.isviewer_output()
    }

    /// Compute a deterministic FNV-1a 64-bit hash over the current VI framebuffer stub.
    #[must_use]
    pub fn framebuffer_hash(&self) -> u64 {
        self.bus.vi.framebuffer_hash(&self.bus.rdram)
    }
}
