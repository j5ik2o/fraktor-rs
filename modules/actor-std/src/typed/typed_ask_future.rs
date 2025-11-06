use cellactor_actor_core_rs::typed::TypedAskFutureGeneric;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

/// Standard runtime typed ask future alias.
pub type TypedAskFuture<M> = TypedAskFutureGeneric<M, StdToolbox>;
