use fraktor_actor_core_rs::core::typed::TypedAskFutureGeneric;
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Standard runtime typed ask future alias.
pub type TypedAskFuture<M> = TypedAskFutureGeneric<M, StdToolbox>;
