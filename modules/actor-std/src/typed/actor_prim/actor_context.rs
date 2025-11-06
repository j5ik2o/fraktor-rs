use cellactor_actor_core_rs::typed::actor_prim::TypedActorContextGeneric;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;
pub type TypedActorContext<'a> = TypedActorContextGeneric<'a, StdToolbox>;
