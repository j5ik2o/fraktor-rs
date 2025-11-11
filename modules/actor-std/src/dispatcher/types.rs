use fraktor_utils_std_rs::runtime_toolbox::StdToolbox;

/// Shared dispatch type specialised for `StdToolbox`.
pub type DispatchShared = fraktor_actor_core_rs::dispatcher::DispatchSharedGeneric<StdToolbox>;
/// Dispatcher specialised for `StdToolbox`.
pub type Dispatcher = fraktor_actor_core_rs::dispatcher::DispatcherGeneric<StdToolbox>;
