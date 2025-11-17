use fraktor_actor_core_rs::core::dispatcher::DispatchError;
use tokio::runtime::Handle;

use crate::dispatcher::{DispatchExecutor, DispatchShared};

/// Executor that drives a dispatcher on a Tokio runtime handle.
pub struct TokioExecutor {
  handle: Handle,
}

impl TokioExecutor {
  /// Creates a new executor bound to the provided Tokio runtime handle.
  #[must_use]
  pub fn new(handle: Handle) -> Self {
    Self { handle }
  }
}

impl DispatchExecutor for TokioExecutor {
  fn execute(&self, dispatcher: DispatchShared) -> Result<(), DispatchError> {
    #[allow(clippy::let_underscore_future)]
    let _ = self.handle.spawn_blocking(move || dispatcher.drive());
    Ok(())
  }
}
