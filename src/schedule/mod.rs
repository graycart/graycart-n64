//! Deterministic single-threaded event-queue slice scheduler.
//!
//! Master clock unit: VR4300 CPU cycles (~93.75 MHz NTSC).
//! RCP clock runs at an exact `2 / 3` rational ratio accumulated without floating point.
//! COP0 `Count` increments at half CPU rate (`1` tick per `2` CPU cycles); reads
//! include cycles elapsed since the last sync, and `Compare` scheduling is 32-bit
//! wrap-safe (see [`CountClock`]).
//!
//! Cited: Graycart N64 Development Plan §3.3 & N64 Emulation Deep Dive §02
//!   docs/DEVELOPMENT-PLAN.md · docs/01-cpu-r4300i.md

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::rcp::{RCP_RATIO_DEN, RCP_RATIO_NUM};

/// First-class scheduled hardware events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    CompareMatch,
    ViLine,
    PiDmaComplete,
    SiDmaComplete,
    AiDmaStep,
    SpDmaComplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledEvent {
    pub at_cycle: u64,
    pub seq: u64,
    pub kind: EventKind,
}

/// Deterministic event-queue slice scheduler with exact `2/3` RCP rational accumulator.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Scheduler {
    now_cpu_cycles: u64,
    rcp_cycles: u64,
    rcp_rem: u64,
    next_seq: u64,
    events: Vec<ScheduledEvent>,
    /// Debug switch shrinking slices to 1 instruction (§3.3).
    pub single_step_mode: bool,
}

impl Scheduler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.now_cpu_cycles = 0;
        self.rcp_cycles = 0;
        self.rcp_rem = 0;
        self.next_seq = 0;
        self.events.clear();
    }

    #[must_use]
    pub fn now_cpu_cycles(&self) -> u64 {
        self.now_cpu_cycles
    }

    #[must_use]
    pub fn rcp_cycles(&self) -> u64 {
        self.rcp_cycles
    }

    /// Advance the master clock by `cpu_cycles` and accumulate exact `2/3` RCP cycles.
    ///
    /// Returns the number of whole RCP cycles elapsed during this step.
    pub fn advance(&mut self, cpu_cycles: u64) -> u64 {
        self.now_cpu_cycles = self.now_cpu_cycles.saturating_add(cpu_cycles);
        let num = cpu_cycles
            .saturating_mul(RCP_RATIO_NUM)
            .saturating_add(self.rcp_rem);
        let rcp_delta = num / RCP_RATIO_DEN;
        self.rcp_rem = num % RCP_RATIO_DEN;
        self.rcp_cycles = self.rcp_cycles.saturating_add(rcp_delta);
        rcp_delta
    }

    /// Schedule an event at absolute master cycle `at_cycle` (replacing any prior
    /// pending event of the same `kind`).
    pub fn schedule(&mut self, at_cycle: u64, kind: EventKind) {
        self.cancel(kind);
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.events.push(ScheduledEvent {
            at_cycle,
            seq,
            kind,
        });
        self.events.sort_by_key(|e| (e.at_cycle, e.seq));
    }

    /// Schedule an event `delay_cycles` after `now_cpu_cycles`.
    pub fn schedule_after(&mut self, delay_cycles: u64, kind: EventKind) {
        self.schedule(self.now_cpu_cycles.saturating_add(delay_cycles), kind);
    }

    /// Remove any pending event matching `kind`.
    pub fn cancel(&mut self, kind: EventKind) {
        self.events.retain(|e| e.kind != kind);
    }

    /// Peek at the earliest pending event timestamp, if any.
    #[must_use]
    pub fn next_event_cycle(&self) -> Option<u64> {
        self.events.first().map(|e| e.at_cycle)
    }

    /// Compute the next CPU execution slice budget capped by `max_budget`,
    /// the nearest scheduled event, and `single_step_mode`.
    #[must_use]
    pub fn next_slice_budget(&self, max_budget: u64) -> u64 {
        if self.single_step_mode {
            return 1.min(max_budget);
        }
        let until_event = self
            .next_event_cycle()
            .map(|t| t.saturating_sub(self.now_cpu_cycles))
            .unwrap_or(max_budget);
        max_budget.min(until_event).max(1)
    }

    /// Pop the next event whose timestamp is `<= now_cpu_cycles`, if any.
    pub fn pop_due(&mut self) -> Option<ScheduledEvent> {
        if self
            .events
            .first()
            .is_some_and(|e| e.at_cycle <= self.now_cpu_cycles)
        {
            Some(self.events.remove(0))
        } else {
            None
        }
    }
}

/// Sync-aware, 32-bit wrap-safe VR4300 COP0 `Count`/`Compare` timer model (§3.3).
///
/// `Count` increments at half CPU speed (every 2 CPU cycles). `read_count`
/// always incorporates cycles elapsed since the last sync point so timers
/// written behind an un-synced `Count` can never stall for a 91-second wrap.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CountClock {
    base_count: u32,
    base_cpu_cycle: u64,
    compare: u32,
    pub ip7_pending: bool,
}

impl CountClock {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Read current `Count` at `now_cpu_cycles` (half CPU rate, wrap-safe).
    #[must_use]
    pub fn read_count(&self, now_cpu_cycles: u64) -> u32 {
        let elapsed_ticks = now_cpu_cycles.saturating_sub(self.base_cpu_cycle) / 2;
        self.base_count.wrapping_add(elapsed_ticks as u32)
    }

    #[must_use]
    pub fn compare(&self) -> u32 {
        self.compare
    }

    /// Write `Count` (`MTC0 Count`) and reschedule [`EventKind::CompareMatch`].
    pub fn write_count(&mut self, value: u32, scheduler: &mut Scheduler) {
        self.base_count = value;
        self.base_cpu_cycle = scheduler.now_cpu_cycles();
        self.reschedule_compare(scheduler);
    }

    /// Write `Compare` (`MTC0 Compare`): clears IP7 and reschedules wrap-safely.
    pub fn write_compare(&mut self, value: u32, scheduler: &mut Scheduler) {
        let now = scheduler.now_cpu_cycles();
        let current = self.read_count(now);
        // Preserve the fractional half-cycle phase so sub-tick cadence never drifts.
        let elapsed_cpu = now.saturating_sub(self.base_cpu_cycle);
        self.base_count = current;
        self.base_cpu_cycle = now - (elapsed_cpu & 1);
        self.compare = value;
        self.ip7_pending = false;
        self.reschedule_compare(scheduler);
    }

    /// Called when [`EventKind::CompareMatch`] fires on the scheduler.
    pub fn on_compare_event(&mut self, scheduler: &mut Scheduler) {
        self.ip7_pending = true;
        // Next match occurs after a full 32-bit Count wrap (2^32 ticks = 2^33 CPU cycles).
        let now = scheduler.now_cpu_cycles();
        let current = self.read_count(now);
        let elapsed_cpu = now.saturating_sub(self.base_cpu_cycle);
        self.base_count = current;
        self.base_cpu_cycle = now - (elapsed_cpu & 1);
        self.reschedule_compare(scheduler);
    }

    fn reschedule_compare(&self, scheduler: &mut Scheduler) {
        let now = scheduler.now_cpu_cycles();
        let current = self.read_count(now);
        let diff_ticks = u64::from(self.compare.wrapping_sub(current));
        // If Compare == current Count, the next match happens after a full 2^32 tick wrap.
        let ticks_until_match = if diff_ticks == 0 {
            1u64 << 32
        } else {
            diff_ticks
        };
        // Account for odd half-cycle phase relative to `base_cpu_cycle`.
        let phase = now.saturating_sub(self.base_cpu_cycle) & 1;
        let cpu_delay = ticks_until_match * 2 - phase;
        scheduler.schedule_after(cpu_delay, EventKind::CompareMatch);
    }
}
