use fraktor_utils_std_rs::runtime_toolbox::StdToolbox;

/// Future primitive specialised for `StdToolbox`.
pub type ActorFuture<T> = fraktor_actor_core_rs::futures::ActorFuture<T, StdToolbox>;
/// Future listener specialised for `StdToolbox`.
pub type ActorFutureListener<'a, T> = fraktor_actor_core_rs::futures::ActorFutureListener<'a, T, StdToolbox>;
