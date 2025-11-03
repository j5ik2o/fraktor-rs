use cellactor_utils_std_rs::StdToolbox;

/// Future primitive specialised for `StdToolbox`.
pub type ActorFuture<T> = cellactor_actor_core_rs::futures::ActorFuture<T, StdToolbox>;
/// Future listener specialised for `StdToolbox`.
pub type ActorFutureListener<'a, T> = cellactor_actor_core_rs::futures::ActorFutureListener<'a, T, StdToolbox>;
