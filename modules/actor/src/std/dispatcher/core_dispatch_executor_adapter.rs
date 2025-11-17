use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

use super::{DispatchExecutor, DispatchShared};
use crate::core::dispatcher::{DispatchError, DispatchExecutor as CoreDispatchExecutor};

/// Adapter bridging core [`CoreDispatchExecutor`] trait objects to the standard executor trait.
pub struct CoreDispatchExecutorAdapter {
  inner: ArcShared<dyn CoreDispatchExecutor<StdToolbox>>,
}

impl CoreDispatchExecutorAdapter {
  /// Creates a new adapter wrapping the given core executor.
  #[must_use]
  pub const fn new(inner: ArcShared<dyn CoreDispatchExecutor<StdToolbox>>) -> Self {
    Self { inner }
  }
}

impl DispatchExecutor for CoreDispatchExecutorAdapter {
  fn execute(&self, dispatcher: DispatchShared) -> Result<(), DispatchError> {
    self.inner.execute(dispatcher)
  }
}
