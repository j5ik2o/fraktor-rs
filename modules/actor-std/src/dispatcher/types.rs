use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Shared dispatch type specialised for `StdToolbox`.
pub type DispatchShared = fraktor_actor_core_rs::core::dispatcher::DispatchSharedGeneric<StdToolbox>;
/// Dispatcher specialised for `StdToolbox`.
pub type Dispatcher = fraktor_actor_core_rs::core::dispatcher::DispatcherGeneric<StdToolbox>;
