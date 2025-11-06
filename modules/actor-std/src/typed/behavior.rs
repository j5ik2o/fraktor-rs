use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

/// Convenience alias for typed behaviors bound to the standard runtime toolbox.
pub type Behavior<M> = cellactor_actor_core_rs::typed::Behavior<M, StdToolbox>;
