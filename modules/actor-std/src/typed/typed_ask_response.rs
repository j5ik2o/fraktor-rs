use cellactor_actor_core_rs::typed::TypedAskResponseGeneric;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

pub type TypedAskResponse<R> = TypedAskResponseGeneric<R, StdToolbox>;
