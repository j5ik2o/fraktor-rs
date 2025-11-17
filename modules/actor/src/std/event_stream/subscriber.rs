use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use super::EventStreamEvent;
use crate::core::event_stream::EventStreamSubscriber as CoreEventStreamSubscriber;

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
