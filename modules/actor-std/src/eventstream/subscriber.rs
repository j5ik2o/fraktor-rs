use cellactor_actor_core_rs::eventstream::EventStreamSubscriber as CoreEventStreamSubscriber;
use cellactor_utils_std_rs::StdToolbox;

use super::EventStreamEvent;

/// Trait implemented by observers interested in the standard runtime event stream.
pub trait EventStreamSubscriber: Send + Sync + 'static {
  /// Receives a published event.
  fn on_event(&self, event: &EventStreamEvent);
}

impl<T> EventStreamSubscriber for T
where
  T: CoreEventStreamSubscriber<StdToolbox>,
{
  fn on_event(&self, event: &EventStreamEvent) {
    CoreEventStreamSubscriber::on_event(self, event)
  }
}
