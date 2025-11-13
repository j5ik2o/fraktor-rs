use alloc::vec::Vec;
use core::time::Duration;

use proptest::prelude::*;

use super::TimerWheel;
use crate::time::{
  ClockKind, DriftMonitor, DriftStatus, ManualClock, MonotonicClock, TimerEntry, TimerInstant, TimerWheelConfig,
};

#[test]
fn fifo_order_within_same_tick() {
  let resolution = Duration::from_millis(10);
  let config = TimerWheelConfig::new(resolution, 128, 5);
  let mut wheel = TimerWheel::new(config);
  let clock = ManualClock::new(resolution);

  let deadline = clock.now().saturating_add_ticks(3);
  for payload in 0u32..10 {
    let entry = TimerEntry::oneshot(deadline, payload);
    wheel.schedule(entry).expect("schedule succeeds");
  }

  let expired = wheel.collect_expired(deadline);
  let payloads: Vec<u32> = expired.into_iter().map(TimerEntry::into_payload).collect();
  assert_eq!(payloads, (0u32..10).collect::<Vec<_>>());
}

#[test]
fn drift_monitor_emits_warning_when_budget_exceeded() {
  let resolution = Duration::from_millis(5);
  let config = TimerWheelConfig::new(resolution, 64, 5);
  let anchor = TimerInstant::zero(resolution);
  let mut monitor = DriftMonitor::new(config, anchor);

  let deadline = anchor.saturating_add_ticks(1);
  let actual = TimerInstant::from_ticks(2, resolution);
  let status = monitor.observe(deadline, actual);
  assert!(matches!(status, DriftStatus::Exceeded { .. }));
  assert!(monitor.last_exceeded().is_some());
}

#[test]
fn manual_clock_is_monotonic() {
  let resolution = Duration::from_millis(1);
  let mut clock = ManualClock::new(resolution);
  assert_eq!(clock.kind(), ClockKind::Deterministic);

  let mut last = clock.now();
  for step in 1..=10u32 {
    let next = clock.advance(Duration::from_millis(step.into()));
    assert!(next >= last);
    last = next;
  }
}

proptest! {
  #[test]
  fn fifo_is_preserved_under_random_delays(delays in proptest::collection::vec(1u8..8, 4..16)) {
    let resolution = Duration::from_millis(2);
    let config = TimerWheelConfig::new(resolution, 256, 5);
    let mut wheel = TimerWheel::new(config);

    let mut tick = TimerInstant::zero(resolution);
    for (idx, delta) in delays.iter().copied().enumerate() {
      tick = tick.saturating_add_ticks(delta.into());
      let entry = TimerEntry::oneshot(tick, idx);
      wheel.schedule(entry).expect("schedule succeeds");
    }

    let expired = wheel.collect_expired(tick);
    let mut last_tick = 0u64;
    let mut last_idx = 0usize;
    for entry in expired {
      let current_tick = entry.deadline().ticks();
      let payload = entry.into_payload();
      assert!(current_tick >= last_tick);
      if current_tick == last_tick {
        assert!(payload >= last_idx);
      }
      last_tick = current_tick;
      last_idx = payload;
    }
  }
}
