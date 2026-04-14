//! Tick driver bundle unit tests.

use core::time::Duration;

use crate::core::kernel::actor::scheduler::tick_driver::{
  AutoDriverMetadata, AutoProfileKind, TickDriverBundle, TickDriverId, TickDriverKind, TickExecutorSignal, TickFeed,
};

fn runtime_bundle() -> TickDriverBundle {
  let feed = TickFeed::new(Duration::from_millis(1), 1, TickExecutorSignal::new());
  let metadata = AutoDriverMetadata {
    profile:    AutoProfileKind::Custom,
    driver_id:  TickDriverId::new(1),
    resolution: Duration::from_millis(1),
  };
  TickDriverBundle::new(TickDriverId::new(1), TickDriverKind::Auto, Duration::from_millis(1), feed)
    .with_auto_metadata(metadata)
}

#[test]
fn clone_preserves_feed_and_auto_metadata() {
  let bundle = runtime_bundle();
  let cloned = bundle.clone();

  assert!(cloned.feed().is_some());
  assert_eq!(cloned.auto_metadata().map(|metadata| metadata.profile), Some(AutoProfileKind::Custom));
}

#[test]
fn noop_bundle_has_no_feed() {
  let bundle = TickDriverBundle::noop(TickDriverId::new(1), TickDriverKind::Auto, Duration::from_millis(1));
  assert!(bundle.feed().is_none());
}

#[test]
fn getters_return_correct_values() {
  let bundle = runtime_bundle();
  assert_eq!(bundle.id(), TickDriverId::new(1));
  assert_eq!(bundle.kind(), TickDriverKind::Auto);
  assert_eq!(bundle.resolution(), Duration::from_millis(1));
}
