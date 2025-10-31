use cellactor_actor_core_rs::DispatcherConfig;
use cellactor_utils_core_rs::sync::ArcShared;
use tokio::runtime::{Handle, TryCurrentError};

use crate::TokioDispatchExecutor;

/// Extension helpers that construct [`DispatcherConfig`] backed by a Tokio runtime.
pub trait TokioDispatcherConfigExt: Sized {
  /// Creates a dispatcher configuration from the provided Tokio [`Handle`].
  fn from_tokio_handle(handle: Handle) -> Self;

  /// Attempts to create a dispatcher configuration from the current Tokio runtime.
  ///
  /// # Errors
  ///
  /// Returns [`TryCurrentError`] when this function is invoked outside of a running Tokio runtime.
  fn try_tokio_current() -> Result<Self, TryCurrentError>;

  /// Creates a dispatcher configuration from the current Tokio runtime.
  ///
  /// # Panics
  ///
  /// Panics when no Tokio runtime is running on the current thread.
  fn tokio_current() -> Self {
    match Self::try_tokio_current() {
      | Ok(config) => config,
      | Err(_) => panic!("Tokio runtime handle is not available"),
    }
  }
}

impl TokioDispatcherConfigExt for DispatcherConfig {
  fn from_tokio_handle(handle: Handle) -> Self {
    DispatcherConfig::from_executor(ArcShared::new(TokioDispatchExecutor::new(handle)))
  }

  fn try_tokio_current() -> Result<Self, TryCurrentError> {
    Handle::try_current().map(Self::from_tokio_handle)
  }
}

#[cfg(test)]
mod tests;
