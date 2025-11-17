use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Convenience alias for typed behaviors bound to the standard runtime toolbox.
pub type Behavior<M> = crate::core::typed::Behavior<M, StdToolbox>;

/// Alias for the supervision builder returned by `Behaviors::supervise`.
pub type Supervise<M> = crate::core::typed::Supervise<M, StdToolbox>;
