use core::marker::PhantomData;

use cellactor_utils_core_rs::sync::NoStdToolbox;

use super::{
  dispatch_error::DispatchError, dispatch_executor::DispatchExecutor, dispatch_shared::DispatchSharedGeneric,
};
use crate::RuntimeToolbox;

/// Simple executor that runs tasks immediately in a synchronous context.
pub struct InlineExecutorGeneric<TB: RuntimeToolbox + 'static> {
  _marker: PhantomData<TB>,
}

/// Type alias for `InlineExecutorGeneric` with the default `NoStdToolbox`.
pub type InlineExecutor = InlineExecutorGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> Default for InlineExecutorGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}

impl<TB: RuntimeToolbox + 'static> InlineExecutorGeneric<TB> {
  #[must_use]
  /// Returns an executor that runs tasks on the calling thread.
  pub const fn new() -> Self {
    Self { _marker: PhantomData }
  }
}

impl<TB> DispatchExecutor<TB> for InlineExecutorGeneric<TB>
where
  TB: RuntimeToolbox + Send + Sync + 'static,
{
  fn execute(&self, dispatcher: DispatchSharedGeneric<TB>) -> Result<(), DispatchError> {
    dispatcher.drive();
    Ok(())
  }

  fn supports_blocking(&self) -> bool {
    false
  }
}
