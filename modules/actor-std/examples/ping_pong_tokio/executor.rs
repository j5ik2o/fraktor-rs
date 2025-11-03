use cellactor_actor_core_rs::system::dispatcher::{DispatchExecutor, DispatchHandle};
use cellactor_actor_std_rs::StdToolbox;
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

impl DispatchExecutor<StdToolbox> for TokioExecutor {
  fn execute(&self, dispatcher: DispatchHandle<StdToolbox>) {
    let _ = self.handle.spawn_blocking(move || dispatcher.drive());
  }
}
