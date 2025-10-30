use super::{dispatch_executor::DispatchExecutor, dispatch_handle::DispatchHandle};

/// 同期コンテキスト上で即時に実行するシンプルなエグゼキュータ。
pub struct InlineExecutor;

impl InlineExecutor {
  #[must_use]
  /// Returns an executor that runs tasks on the calling thread.
  pub const fn new() -> Self {
    Self
  }
}

impl DispatchExecutor for InlineExecutor {
  fn execute(&self, dispatcher: DispatchHandle) {
    dispatcher.drive();
  }
}
