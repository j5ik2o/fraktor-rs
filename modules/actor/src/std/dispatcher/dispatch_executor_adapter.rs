use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

use super::{DispatchExecutor, DispatchShared};
use crate::core::dispatcher::{DispatchError, DispatchExecutor as CoreDispatchExecutor};

/// Adapter bridging [`DispatchExecutor`] trait objects to the core runtime.
pub struct DispatchExecutorAdapter {
  inner: ArcShared<dyn DispatchExecutor>,
}

impl DispatchExecutorAdapter {
  /// Creates a new adapter wrapping the given executor.
  #[must_use]
  pub const fn new(inner: ArcShared<dyn DispatchExecutor>) -> Self {
    Self { inner }
  }
}

impl CoreDispatchExecutor<StdToolbox> for DispatchExecutorAdapter {
  fn execute(&self, dispatcher: DispatchShared) -> Result<(), DispatchError> {
    self.inner.execute(dispatcher)
  }
}
