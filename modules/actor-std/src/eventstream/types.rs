use cellactor_actor_core_rs::eventstream::{
  EventStreamEvent as CoreEventStreamEvent, EventStreamGeneric as CoreEventStream,
  EventStreamSubscriptionGeneric as CoreEventStreamSubscription,
};
use cellactor_utils_std_rs::StdToolbox;

/// Event stream specialised for `StdToolbox`.
pub type EventStream = CoreEventStream<StdToolbox>;
/// Event stream event specialised for `StdToolbox`.
pub type EventStreamEvent = CoreEventStreamEvent<StdToolbox>;
/// Event stream subscription specialised for `StdToolbox`.
pub type EventStreamSubscription = CoreEventStreamSubscription<StdToolbox>;
