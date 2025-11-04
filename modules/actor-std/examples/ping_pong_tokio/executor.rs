use cellactor_actor_std_rs::dispatcher::{DispatchExecutor, DispatchShared};
use tokio::runtime::Handle;

/// Tokio ランタイム上で Dispatcher を駆動する実装。
pub struct TokioExecutor {
  handle: Handle,
}

impl TokioExecutor {
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
