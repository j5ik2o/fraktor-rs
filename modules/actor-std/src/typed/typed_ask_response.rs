use fraktor_actor_core_rs::typed::TypedAskResponseGeneric;
use fraktor_utils_core_rs::std::runtime_toolbox::StdToolbox;

/// Standard runtime typed ask response alias.
pub type TypedAskResponse<R> = TypedAskResponseGeneric<R, StdToolbox>;
