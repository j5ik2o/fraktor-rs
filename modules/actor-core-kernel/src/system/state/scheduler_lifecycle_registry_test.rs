use core::time::Duration;

use portable_atomic::Ordering;

use super::SchedulerLifecycleRegistry;
use crate::{
  actor::scheduler::{
    SchedulerConfig, SchedulerContext,
    tick_driver::{TickDriverBundle, TickDriverKind, TickExecutorSignal, TickFeed, next_tick_driver_id},
  },
  event::stream::{EventStream, EventStreamShared},
};

#[test]
fn scheduler_lifecycle_registry_starts_unstarted_and_unterminated() {
  let event_stream = EventStreamShared::new(EventStream::with_capacity(4));
  let config = SchedulerConfig::default();
  let context = SchedulerContext::with_event_stream(config, event_stream);
  let resolution = Duration::from_millis(1);
  let signal = TickExecutorSignal::new();
  let feed = TickFeed::new(resolution, 1, signal);
  let bundle = TickDriverBundle::new(next_tick_driver_id(), TickDriverKind::Auto, resolution, feed);
  let registry = SchedulerLifecycleRegistry::new(context, bundle);

  assert!(!registry.root_started.load(Ordering::Acquire));
  assert_eq!(registry.start_time, Duration::ZERO);
  assert!(registry.tick_driver_snapshot.is_none());
  assert!(registry.tick_driver_stopper.is_none());
}
