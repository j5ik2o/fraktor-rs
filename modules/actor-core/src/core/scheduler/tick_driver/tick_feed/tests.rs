//! Tick feed unit tests.

use alloc::{vec, vec::Vec};
use core::time::Duration;

use fraktor_utils_core_rs::core::{runtime_toolbox::NoStdToolbox, time::TimerInstant};

use crate::core::scheduler::{TickDriverKind, TickExecutorSignal, TickFeed};

#[test]
fn enqueue_wakes_signal_and_preserves_order() {
  let signal = TickExecutorSignal::new();
  let feed = TickFeed::<NoStdToolbox>::new(Duration::from_millis(1), 4, signal.clone());
  assert!(!signal.arm(), "signal should start idle");
  feed.enqueue(1);
  feed.enqueue(2);
  assert!(signal.arm(), "enqueue should wake signal");
  assert!(!signal.arm(), "arm resets pending flag");

  let mut drained = Vec::new();
  feed.drain_pending(|ticks| drained.push(ticks));
  assert_eq!(drained, vec![1, 2]);
}

#[test]
fn snapshot_reports_dropped_ticks() {
  let signal = TickExecutorSignal::new();
  let feed = TickFeed::<NoStdToolbox>::new(Duration::from_millis(1), 1, signal);
  feed.enqueue(1);
  feed.enqueue(1);
  let now = TimerInstant::from_ticks(1_000, Duration::from_millis(1));
  let metrics = feed.snapshot(now, TickDriverKind::Auto);
  assert!(metrics.dropped_total() > 0);
}
