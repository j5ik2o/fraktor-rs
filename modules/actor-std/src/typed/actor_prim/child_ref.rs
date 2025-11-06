use cellactor_actor_core_rs::typed::actor_prim::TypedChildRefGeneric;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

pub type TypedChildRef<M> = TypedChildRefGeneric<M, StdToolbox>;
