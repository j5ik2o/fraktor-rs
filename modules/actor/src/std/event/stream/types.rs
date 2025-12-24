use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Event stream specialised for `StdToolbox` (shared wrapper).
pub type EventStream = crate::core::event::stream::EventStreamSharedGeneric<StdToolbox>;
/// Event stream event specialised for `StdToolbox`.
pub type EventStreamEvent = crate::core::event::stream::EventStreamEvent<StdToolbox>;
/// Event stream subscription specialised for `StdToolbox`.
pub type EventStreamSubscription = crate::core::event::stream::EventStreamSubscriptionGeneric<StdToolbox>;
