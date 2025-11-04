use cellactor_utils_std_rs::StdToolbox;

/// Shared dispatch type specialised for `StdToolbox`.
pub type DispatchShared = cellactor_actor_core_rs::dispatcher::DispatchShared<StdToolbox>;
/// Dispatcher specialised for `StdToolbox`.
pub type Dispatcher = cellactor_actor_core_rs::dispatcher::Dispatcher<StdToolbox>;
