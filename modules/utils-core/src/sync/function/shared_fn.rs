use crate::sync::{Shared, SharedBound};

/// Trait alias for shared function pointers used in actor-core.
pub trait SharedFn<Args, Output>: Shared<super::SharedFnTarget<Args, Output>> + SharedBound {}

impl<T, Args, Output> SharedFn<Args, Output> for T where T: Shared<super::SharedFnTarget<Args, Output>> + SharedBound {}
