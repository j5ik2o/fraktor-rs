//! Tick driver bundle unit tests.

use alloc::boxed::Box;
use core::{
  sync::atomic::{AtomicBool, AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::kernel::actor::scheduler::tick_driver::{
  AutoDriverMetadata, AutoProfileKind, TickDriverBundle, TickDriverControl, TickDriverControlShared,
  TickDriverHandle, TickDriverId, TickDriverKind, TickExecutorSignal, TickFeed,
};

struct RecordingControl {
  shutdown_calls: ArcShared<AtomicUsize>,
  did_shutdown:   AtomicBool,
}

impl RecordingControl {
  fn new(shutdown_calls: ArcShared<AtomicUsize>) -> Self {
    Self { shutdown_calls, did_shutdown: AtomicBool::new(false) }
  }
}

impl TickDriverControl for RecordingControl {
  fn shutdown(&self) {
    if !self.did_shutdown.swap(true, Ordering::SeqCst) {
      self.shutdown_calls.fetch_add(1, Ordering::SeqCst);
    }
  }
}

fn runtime_bundle(shutdown_calls: ArcShared<AtomicUsize>) -> TickDriverBundle {
  let control: Box<dyn TickDriverControl> = Box::new(RecordingControl::new(shutdown_calls));
  let control = TickDriverControlShared::new(control);
  let handle = TickDriverHandle::new(TickDriverId::new(1), TickDriverKind::Auto, Duration::from_millis(1), control);
  let feed = TickFeed::new(Duration::from_millis(1), 1, TickExecutorSignal::new());
  let metadata = AutoDriverMetadata {
    profile:    AutoProfileKind::Custom,
    driver_id:  TickDriverId::new(1),
    resolution: Duration::from_millis(1),
  };
  TickDriverBundle::new(handle, feed).with_auto_metadata(metadata)
}

#[test]
fn shutdown_delegates_to_driver_control() {
  let shutdown_calls = ArcShared::new(AtomicUsize::new(0));
  let mut bundle = runtime_bundle(shutdown_calls.clone());

  bundle.shutdown();

  assert_eq!(shutdown_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn clone_preserves_feed_and_auto_metadata() {
  let shutdown_calls = ArcShared::new(AtomicUsize::new(0));
  let bundle = runtime_bundle(shutdown_calls);
  let cloned = bundle.clone();

  assert!(cloned.feed().is_some());
  assert_eq!(cloned.auto_metadata().map(|metadata| metadata.profile), Some(AutoProfileKind::Custom));
}
