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
  fn execute(&self, dispatcher: DispatchShared) {
    let _ = self.handle.spawn_blocking(move || dispatcher.drive());
  }
}
