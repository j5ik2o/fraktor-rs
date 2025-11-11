use fraktor_utils_std_rs::runtime_toolbox::StdToolbox;

/// Convenience alias for typed behaviors bound to the standard runtime toolbox.
pub type Behavior<M> = fraktor_actor_core_rs::typed::Behavior<M, StdToolbox>;

/// Alias for the supervision builder returned by `Behaviors::supervise`.
pub type Supervise<M> = fraktor_actor_core_rs::typed::Supervise<M, StdToolbox>;
