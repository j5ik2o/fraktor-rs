use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Shared dispatch type specialised for `StdToolbox`.
pub type DispatchShared = crate::core::dispatcher::DispatchSharedGeneric<StdToolbox>;
/// Dispatcher specialised for `StdToolbox`.
pub type Dispatcher = crate::core::dispatcher::DispatcherGeneric<StdToolbox>;
