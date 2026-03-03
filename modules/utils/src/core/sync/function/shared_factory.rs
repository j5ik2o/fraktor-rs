use crate::core::sync::shared::{Shared, SharedBound};

/// Trait alias for shared factories (Send + Sync closures returning a type).
#[allow(dead_code)]
pub(crate) trait SharedFactory<Args, Output>: Shared<super::SharedFnTarget<Args, Output>> + SharedBound {}

impl<T, Args, Output> SharedFactory<Args, Output> for T where
  T: Shared<super::SharedFnTarget<Args, Output>> + SharedBound
{
}
