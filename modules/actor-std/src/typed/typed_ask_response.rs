use cellactor_actor_core_rs::typed::TypedAskResponseGeneric;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

/// Standard runtime typed ask response alias.
pub type TypedAskResponse<R> = TypedAskResponseGeneric<R, StdToolbox>;
