mod subscriber;
mod subscriber_adapter;
mod types;

use cellactor_actor_core_rs::event_stream::EventStreamSubscriber as CoreEventStreamSubscriber;
use cellactor_utils_core_rs::sync::ArcShared;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;
pub use subscriber::EventStreamSubscriber;
pub use types::*;

use self::subscriber_adapter::EventStreamSubscriberAdapter;
use crate::system::ActorSystem;

/// Subscribes an observer implemented using [`EventStreamSubscriber`].
#[must_use]
pub fn subscribe(system: &ActorSystem, subscriber: &ArcShared<dyn EventStreamSubscriber>) -> EventStreamSubscription {
  let adapter: ArcShared<dyn CoreEventStreamSubscriber<StdToolbox>> =
    ArcShared::new(EventStreamSubscriberAdapter::new(subscriber.clone()));
  system.as_core().subscribe_event_stream(&adapter)
}
