//! Trait implemented by event stream observers.

use crate::{EventStreamEvent, RuntimeToolbox};

/// Observers registered with the event stream must implement this trait.
pub trait EventStreamSubscriber<TB: RuntimeToolbox>: Send + Sync + 'static {
  /// Invoked for every published event.
  fn on_event(&self, event: &EventStreamEvent<TB>);
}
