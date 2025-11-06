use cellactor_actor_core_rs::typed::actor_prim::TypedActorRefGeneric;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

pub type TypedActorRef<M> = TypedActorRefGeneric<M, StdToolbox>;
