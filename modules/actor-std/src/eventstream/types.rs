use cellactor_utils_std_rs::StdToolbox;

/// Event stream specialised for `StdToolbox`.
pub type EventStream = cellactor_actor_core_rs::eventstream::EventStreamGeneric<StdToolbox>;
/// Event stream event specialised for `StdToolbox`.
pub type EventStreamEvent = cellactor_actor_core_rs::eventstream::EventStreamEvent<StdToolbox>;
/// Event stream subscription specialised for `StdToolbox`.
pub type EventStreamSubscription = cellactor_actor_core_rs::eventstream::EventStreamSubscriptionGeneric<StdToolbox>;
