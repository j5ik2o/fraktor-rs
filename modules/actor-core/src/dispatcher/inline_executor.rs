use core::marker::PhantomData;

use super::{dispatch_executor::DispatchExecutor, dispatch_shared::DispatchShared};
use crate::RuntimeToolbox;

/// Simple executor that runs tasks immediately in a synchronous context.
pub struct InlineExecutor<TB: RuntimeToolbox + 'static> {
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> Default for InlineExecutor<TB> {
  fn default() -> Self {
    Self::new()
  }
}

impl<TB: RuntimeToolbox + 'static> InlineExecutor<TB> {
  #[must_use]
  /// Returns an executor that runs tasks on the calling thread.
  pub const fn new() -> Self {
    Self { _marker: PhantomData }
  }
}

impl<TB> DispatchExecutor<TB> for InlineExecutor<TB>
where
  TB: RuntimeToolbox + Send + Sync + 'static,
{
  fn execute(&self, dispatcher: DispatchShared<TB>) {
    dispatcher.drive();
  }
}
