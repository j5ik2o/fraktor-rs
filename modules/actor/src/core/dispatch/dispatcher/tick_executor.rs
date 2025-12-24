use alloc::collections::VecDeque;

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use super::{
  dispatch_error::DispatchError, dispatch_executor::DispatchExecutor, dispatch_shared::DispatchSharedGeneric,
};

/// Executor that queues dispatcher batches until `tick` is invoked.
pub struct TickExecutorGeneric<TB: RuntimeToolbox + 'static> {
  queue: VecDeque<DispatchSharedGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> TickExecutorGeneric<TB> {
  /// Creates an empty tick-driven executor.
  #[must_use]
  pub const fn new() -> Self {
    Self { queue: VecDeque::new() }
  }

  /// Drains all pending dispatcher batches.
  pub fn tick(&mut self) {
    while let Some(task) = self.queue.pop_front() {
      task.drive();
    }
  }

  /// Returns the number of queued tasks (testing helper).
  #[must_use]
  pub fn pending_tasks(&self) -> usize {
    self.queue.len()
  }
}

impl<TB> Default for TickExecutorGeneric<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn default() -> Self {
    Self::new()
  }
}

/// Type alias for the default tick executor.
pub type TickExecutor = TickExecutorGeneric<NoStdToolbox>;

impl<TB> DispatchExecutor<TB> for TickExecutorGeneric<TB>
where
  TB: RuntimeToolbox + Send + Sync + 'static,
{
  fn execute(&mut self, dispatcher: DispatchSharedGeneric<TB>) -> Result<(), DispatchError> {
    self.queue.push_back(dispatcher);
    Ok(())
  }
}
