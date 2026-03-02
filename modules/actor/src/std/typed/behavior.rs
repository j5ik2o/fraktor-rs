/// Convenience alias for typed behaviors bound to the standard runtime toolbox.
pub type Behavior<M> = crate::core::typed::Behavior<M>;

/// Alias for the supervision builder returned by `Behaviors::supervise`.
pub type Supervise<M> = crate::core::typed::Supervise<M>;
