use alloc::boxed::Box;

use fraktor_utils_rs::{
  core::{
    runtime_toolbox::{RuntimeToolbox, SyncMutexFamily},
    sync::ArcShared,
  },
  std::runtime_toolbox::StdToolbox,
};

use super::EventStreamEvent;

/// Trait implemented by observers interested in the standard runtime event stream.
pub trait EventStreamSubscriber: Send + Sync + 'static {
  /// Receives a published event.
  fn on_event(&mut self, event: &EventStreamEvent);
}

/// Shared handle protected by the `StdToolbox` mutex family.
pub type EventStreamSubscriberShared =
  ArcShared<<<StdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::Mutex<Box<dyn EventStreamSubscriber>>>;

/// Wraps the subscriber into a mutex-protected shared handle for the standard runtime.
#[must_use]
pub fn subscriber_handle(subscriber: impl EventStreamSubscriber) -> EventStreamSubscriberShared {
  ArcShared::new(
    <<StdToolbox as fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(
      Box::new(subscriber),
    ),
  )
}
