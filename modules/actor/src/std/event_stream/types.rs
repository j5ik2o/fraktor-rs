use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Event stream specialised for `StdToolbox`.
pub type EventStream = crate::core::event_stream::EventStreamGeneric<StdToolbox>;
/// Event stream event specialised for `StdToolbox`.
pub type EventStreamEvent = crate::core::event_stream::EventStreamEvent<StdToolbox>;
/// Event stream subscription specialised for `StdToolbox`.
pub type EventStreamSubscription = crate::core::event_stream::EventStreamSubscriptionGeneric<StdToolbox>;
