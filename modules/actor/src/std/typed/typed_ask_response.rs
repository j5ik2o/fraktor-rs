use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::core::typed::TypedAskResponseGeneric;

/// Standard runtime typed ask response alias.
pub type TypedAskResponse<R> = TypedAskResponseGeneric<R, StdToolbox>;
