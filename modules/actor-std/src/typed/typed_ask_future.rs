use fraktor_actor_core_rs::typed::TypedAskFutureGeneric;
use fraktor_utils_core_rs::std::runtime_toolbox::StdToolbox;

/// Standard runtime typed ask future alias.
pub type TypedAskFuture<M> = TypedAskFutureGeneric<M, StdToolbox>;
