//! Trait implemented by event stream observers.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::ArcShared,
};

use crate::core::event_stream::EventStreamEvent;

/// Shared subscriber handle guarded by the runtime mutex family.
pub type EventStreamSubscriberShared<TB = NoStdToolbox> =
  ArcShared<ToolboxMutex<Box<dyn EventStreamSubscriber<TB>>, TB>>;

/// Observers registered with the event stream must implement this trait.
pub trait EventStreamSubscriber<TB: RuntimeToolbox = NoStdToolbox>: Send + Sync + 'static {
  /// Invoked for every published event.
  fn on_event(&mut self, event: &EventStreamEvent<TB>);
}

/// Wraps the subscriber into a mutex-protected shared handle.
#[must_use]
pub fn subscriber_handle<TB>(subscriber: impl EventStreamSubscriber<TB>) -> EventStreamSubscriberShared<TB>
where
  TB: RuntimeToolbox + 'static, {
  ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(Box::new(subscriber)))
}
