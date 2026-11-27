//! Deterministic event-queue slice scheduler shell (full event queue, 2/3 RCP
//! rational accumulator, and Count/Compare wrap-safe unit tests land in Task M0.3).
//!
//! Cited: Graycart N64 Development Plan §3.3 — Event-queue slice scheduler
//!   docs/DEVELOPMENT-PLAN.md

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    CompareMatch,
    ViLine,
    PiDmaComplete,
    SiDmaComplete,
    AiDmaStep,
    SpDmaComplete,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Scheduler {
    now_cpu_cycles: u64,
    pub single_step_mode: bool,
}

impl Scheduler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.now_cpu_cycles = 0;
    }

    #[must_use]
    pub fn now_cpu_cycles(&self) -> u64 {
        self.now_cpu_cycles
    }

    pub fn advance(&mut self, cpu_cycles: u64) {
        self.now_cpu_cycles = self.now_cpu_cycles.saturating_add(cpu_cycles);
    }
}
