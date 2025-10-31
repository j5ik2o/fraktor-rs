use cellactor_actor_core_rs::{DispatchExecutor, DispatchHandle};
use tokio::runtime::Handle;

/// [`DispatchExecutor`] implementation that schedules work on a Tokio runtime.
pub struct TokioDispatchExecutor {
  handle: Handle,
}

impl TokioDispatchExecutor {
  /// Creates a new executor using the provided Tokio runtime handle.
  #[must_use]
  pub fn new(handle: Handle) -> Self {
    Self { handle }
  }

  /// Returns the internal Tokio runtime handle.
  #[must_use]
  pub fn handle(&self) -> &Handle {
    &self.handle
  }
}

impl DispatchExecutor for TokioDispatchExecutor {
  fn execute(&self, dispatcher: DispatchHandle) {
    let handle = self.handle.clone();
    let _ = handle.spawn_blocking(move || dispatcher.drive());
  }
}

#[cfg(test)]
mod tests;
