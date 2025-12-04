use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Future primitive specialised for `StdToolbox`.
pub type ActorFuture<T> = crate::core::futures::ActorFuture<T, StdToolbox>;
/// Shared future primitive specialised for `StdToolbox`.
pub type ActorFutureShared<T> = crate::core::futures::ActorFutureSharedGeneric<T, StdToolbox>;
/// Future listener specialised for `StdToolbox`.
pub type ActorFutureListener<T> = crate::core::futures::ActorFutureListener<T, StdToolbox>;
