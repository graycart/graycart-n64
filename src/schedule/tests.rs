//! Unit tests for the deterministic event-queue scheduler and Count/Compare
//! wrap-safe semantics (§3.3 mandatory pre-scheduler test).

use super::*;

#[test]
fn rcp_rational_accumulator_has_zero_drift_across_uneven_steps() {
    let mut sched = Scheduler::new();
    let pattern = [1u64, 2, 5, 7, 13, 1, 3, 11];
    let mut total_cpu = 0u64;
    for i in 0..100_000usize {
        let step = pattern[i % pattern.len()];
        total_cpu += step;
        sched.advance(step);
    }
    assert_eq!(sched.now_cpu_cycles(), total_cpu);
    assert_eq!(sched.rcp_cycles(), (total_cpu * 2) / 3);
}

#[test]
fn events_fire_in_deterministic_order_and_respect_single_step_mode() {
    let mut sched = Scheduler::new();
    sched.schedule(100, EventKind::ViLine);
    sched.schedule(50, EventKind::PiDmaComplete);
    sched.schedule(50, EventKind::SiDmaComplete);

    assert_eq!(sched.next_slice_budget(1000), 50);

    sched.single_step_mode = true;
    assert_eq!(sched.next_slice_budget(1000), 1);
    sched.single_step_mode = false;

    sched.advance(49);
    assert!(sched.pop_due().is_none());

    sched.advance(1);
    assert_eq!(
        sched.pop_due().map(|e| e.kind),
        Some(EventKind::PiDmaComplete)
    );
    assert_eq!(
        sched.pop_due().map(|e| e.kind),
        Some(EventKind::SiDmaComplete)
    );
    assert!(sched.pop_due().is_none());

    sched.advance(50);
    assert_eq!(sched.pop_due().map(|e| e.kind), Some(EventKind::ViLine));
}

#[test]
fn count_reads_include_mid_slice_cycles_and_compare_handles_32bit_wrap() {
    let mut sched = Scheduler::new();
    let mut clock = CountClock::new();

    // Advance 101 CPU cycles mid-slice without syncing: Count must read 50 (101 / 2).
    sched.advance(101);
    assert_eq!(clock.read_count(sched.now_cpu_cycles()), 50);

    // Program Compare = 60 while Count is 50 at odd CPU cycle 101:
    // Next Count tick (51) lands at cycle 102, so Count == 60 lands at cycle 120 (19 cycles later).
    clock.write_compare(60, &mut sched);
    assert!(!clock.ip7_pending);
    assert_eq!(sched.next_event_cycle(), Some(120));

    sched.advance(18); // cycle 119 -> Count = 59
    assert_eq!(clock.read_count(sched.now_cpu_cycles()), 59);
    assert!(sched.pop_due().is_none());

    sched.advance(1); // cycle 120 -> Count = 60 -> CompareMatch fires!
    assert_eq!(clock.read_count(sched.now_cpu_cycles()), 60);
    let ev = sched
        .pop_due()
        .expect("CompareMatch must fire at cycle 120");
    assert_eq!(ev.kind, EventKind::CompareMatch);
    clock.on_compare_event(&mut sched);
    assert!(clock.ip7_pending);

    // Writing Compare clears IP7 and handles 32-bit wrap across 0xFFFF_FFFF -> 0x0000_0005.
    clock.write_count(0xFFFF_FFFE, &mut sched);
    clock.write_compare(0x0000_0005, &mut sched);
    assert!(!clock.ip7_pending);
    // Distance across wrap: 7 Count ticks = 14 CPU cycles.
    let expected_at = sched.now_cpu_cycles() + 14;
    assert_eq!(sched.next_event_cycle(), Some(expected_at));

    sched.advance(14);
    assert_eq!(clock.read_count(sched.now_cpu_cycles()), 0x0000_0005);
    let wrapped_ev = sched.pop_due().expect("wrapped CompareMatch must fire");
    assert_eq!(wrapped_ev.kind, EventKind::CompareMatch);

    // Writing Compare == current Count must schedule a full 2^32-tick wrap, never fire immediately.
    clock.write_compare(0x0000_0005, &mut sched);
    let full_wrap_cycle = sched.now_cpu_cycles() + (1u64 << 33);
    assert_eq!(sched.next_event_cycle(), Some(full_wrap_cycle));
    assert!(sched.pop_due().is_none());
}
