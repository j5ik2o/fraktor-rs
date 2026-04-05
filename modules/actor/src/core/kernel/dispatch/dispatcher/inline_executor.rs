use core::marker::PhantomData;

use super::{dispatch_error::DispatchError, dispatch_executor::DispatchExecutor, dispatch_shared::DispatchShared};

/// Simple executor that runs tasks immediately in a synchronous context.
pub(crate) struct InlineExecutor {
  _marker: PhantomData<()>,
}

impl Default for InlineExecutor {
  fn default() -> Self {
    Self::new()
  }
}

impl InlineExecutor {
  #[must_use]
  /// Returns an executor that runs tasks on the calling thread.
  pub(crate) const fn new() -> Self {
    Self { _marker: PhantomData }
  }
}

impl DispatchExecutor for InlineExecutor {
  fn execute(&mut self, dispatcher: DispatchShared) -> Result<(), DispatchError> {
    dispatcher.drive();
    Ok(())
  }

  fn supports_blocking(&self) -> bool {
    false
  }
}
