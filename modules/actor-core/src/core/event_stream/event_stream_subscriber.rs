//! Trait implemented by event stream observers.

use fraktor_utils_core_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::event_stream::EventStreamEvent;

/// Observers registered with the event stream must implement this trait.
pub trait EventStreamSubscriber<TB: RuntimeToolbox = NoStdToolbox>: Send + Sync + 'static {
  /// Invoked for every published event.
  fn on_event(&self, event: &EventStreamEvent<TB>);
}
