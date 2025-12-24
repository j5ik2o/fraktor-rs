use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Shared dispatch type specialised for `StdToolbox`.
pub type DispatchShared = crate::core::dispatch::dispatcher::DispatchSharedGeneric<StdToolbox>;
/// Dispatcher shared handle specialised for `StdToolbox`.
pub type DispatcherShared = crate::core::dispatch::dispatcher::DispatcherSharedGeneric<StdToolbox>;
