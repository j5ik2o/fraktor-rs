use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Event stream specialised for `StdToolbox`.
pub type EventStream = fraktor_actor_core_rs::core::event_stream::EventStreamGeneric<StdToolbox>;
/// Event stream event specialised for `StdToolbox`.
pub type EventStreamEvent = fraktor_actor_core_rs::core::event_stream::EventStreamEvent<StdToolbox>;
/// Event stream subscription specialised for `StdToolbox`.
pub type EventStreamSubscription =
  fraktor_actor_core_rs::core::event_stream::EventStreamSubscriptionGeneric<StdToolbox>;
