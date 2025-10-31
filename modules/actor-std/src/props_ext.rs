use cellactor_actor_core_rs::{DispatcherConfig, Props};
use tokio::runtime::{Handle, TryCurrentError};

use crate::TokioDispatcherConfigExt;

/// Extension helpers that attach Tokio-backed dispatchers to [`Props`].
pub trait TokioPropsExt: Sized {
  /// Applies a Tokio dispatcher created from the provided [`Handle`].
  fn with_tokio_dispatcher(self, handle: Handle) -> Self;

  /// Applies a Tokio dispatcher created from the current runtime.
  ///
  /// # Errors
  ///
  /// Returns [`TryCurrentError`] when no Tokio runtime is active on the current thread.
  fn try_with_tokio_dispatcher(self) -> Result<Self, TryCurrentError>;

  /// Applies a Tokio dispatcher created from the current runtime.
  ///
  /// # Panics
  ///
  /// Panics when no Tokio runtime is active on the current thread.
  fn with_tokio_dispatcher_current(self) -> Self {
    match self.try_with_tokio_dispatcher() {
      | Ok(props) => props,
      | Err(_) => panic!("Tokio runtime handle is not available"),
    }
  }
}

impl TokioPropsExt for Props {
  fn with_tokio_dispatcher(self, handle: Handle) -> Self {
    let config = DispatcherConfig::from_tokio_handle(handle);
    self.with_dispatcher(config)
  }

  fn try_with_tokio_dispatcher(self) -> Result<Self, TryCurrentError> {
    DispatcherConfig::try_tokio_current().map(|config| self.with_dispatcher(config))
  }
}

#[cfg(test)]
mod tests;
