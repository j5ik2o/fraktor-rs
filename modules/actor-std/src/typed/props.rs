use cellactor_actor_core_rs::typed::TypedPropsGeneric as CoreTypedPropsGeneric;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

pub type TypedProps<M> = CoreTypedPropsGeneric<M, StdToolbox>;
