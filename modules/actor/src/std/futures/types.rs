use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Future primitive specialised for `StdToolbox`.
pub type ActorFuture<T> = crate::core::futures::ActorFuture<T, StdToolbox>;
/// Future listener specialised for `StdToolbox`.
pub type ActorFutureListener<'a, T> = crate::core::futures::ActorFutureListener<'a, T, StdToolbox>;
