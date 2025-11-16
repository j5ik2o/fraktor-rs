//! Tests for [`SchedulerTickMetricsProbe`].

use core::time::Duration;

use fraktor_utils_core_rs::time::TimerInstant;

use crate::{
  NoStdToolbox,
  scheduler::{SchedulerTickMetricsProbe, TickDriverKind, TickExecutorSignal, TickFeed},
};

#[test]
fn snapshot_reports_tick_rate() {
  let signal = TickExecutorSignal::new();
  let feed = TickFeed::<NoStdToolbox>::new(Duration::from_millis(1), 16, signal);
  let probe = SchedulerTickMetricsProbe::new(feed.clone(), Duration::from_millis(1), TickDriverKind::Auto);
  for _ in 0..5 {
    feed.enqueue(1);
  }
  let now = TimerInstant::from_ticks(1_000, Duration::from_millis(1));
  let metrics = probe.snapshot(now);
  assert_eq!(metrics.ticks_per_sec(), 5);
  assert_eq!(metrics.driver(), TickDriverKind::Auto);
}
