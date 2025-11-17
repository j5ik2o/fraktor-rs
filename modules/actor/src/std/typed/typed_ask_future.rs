use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::core::typed::TypedAskFutureGeneric;

/// Standard runtime typed ask future alias.
pub type TypedAskFuture<M> = TypedAskFutureGeneric<M, StdToolbox>;
