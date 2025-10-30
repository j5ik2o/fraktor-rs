use super::{dispatch_executor::DispatchExecutor, dispatch_handle::DispatchHandle};

/// Simple executor that runs tasks immediately in a synchronous context.
pub struct InlineExecutor;

impl Default for InlineExecutor {
  fn default() -> Self {
    Self::new()
  }
}

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
