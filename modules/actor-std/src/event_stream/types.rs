use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

/// Event stream specialised for `StdToolbox`.
pub type EventStream = cellactor_actor_core_rs::event_stream::EventStream<StdToolbox>;
/// Event stream event specialised for `StdToolbox`.
pub type EventStreamEvent = cellactor_actor_core_rs::event_stream::EventStreamEvent<StdToolbox>;
/// Event stream subscription specialised for `StdToolbox`.
pub type EventStreamSubscription = cellactor_actor_core_rs::event_stream::EventStreamSubscription<StdToolbox>;
