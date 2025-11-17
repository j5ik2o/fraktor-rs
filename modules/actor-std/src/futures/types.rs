use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Future primitive specialised for `StdToolbox`.
pub type ActorFuture<T> = fraktor_actor_core_rs::core::futures::ActorFuture<T, StdToolbox>;
/// Future listener specialised for `StdToolbox`.
pub type ActorFutureListener<'a, T> = fraktor_actor_core_rs::core::futures::ActorFutureListener<'a, T, StdToolbox>;
