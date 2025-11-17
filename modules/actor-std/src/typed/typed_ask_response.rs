use fraktor_actor_core_rs::core::typed::TypedAskResponseGeneric;
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Standard runtime typed ask response alias.
pub type TypedAskResponse<R> = TypedAskResponseGeneric<R, StdToolbox>;
