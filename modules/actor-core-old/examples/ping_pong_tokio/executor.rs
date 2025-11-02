use cellactor_actor_core_rs::{DispatchExecutor, DispatchHandle};
use tokio::runtime::Handle;

/// Executor that schedules dispatcher work onto a Tokio runtime.
pub struct TokioExecutor {
  handle: Handle,
}

impl TokioExecutor {
  #[must_use]
  /// Creates a new Tokio-backed executor.
  pub fn new(handle: Handle) -> Self {
    Self { handle }
  }
}

impl DispatchExecutor for TokioExecutor {
  fn execute(&self, dispatcher: DispatchHandle) {
    let _ = self.handle.spawn_blocking(move || dispatcher.drive());
  }
}
