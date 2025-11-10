use alloc::collections::VecDeque;

use cellactor_utils_core_rs::{runtime_toolbox::NoStdToolbox, sync::NoStdMutex};

use super::{
  dispatch_error::DispatchError, dispatch_executor::DispatchExecutor, dispatch_shared::DispatchSharedGeneric,
};
use crate::RuntimeToolbox;

/// Executor that queues dispatcher batches until `tick` is invoked.
pub struct TickExecutorGeneric<TB: RuntimeToolbox + 'static> {
  queue: NoStdMutex<VecDeque<DispatchSharedGeneric<TB>>>,
}

impl<TB: RuntimeToolbox + 'static> TickExecutorGeneric<TB> {
  /// Creates an empty tick-driven executor.
  #[must_use]
  pub const fn new() -> Self {
    Self { queue: NoStdMutex::new(VecDeque::new()) }
  }

  /// Drains all pending dispatcher batches.
  pub fn tick(&self) {
    loop {
      let dispatcher = {
        let mut queue = self.queue.lock();
        queue.pop_front()
      };
      match dispatcher {
        | Some(task) => task.drive(),
        | None => break,
      }
    }
  }

  /// Returns the number of queued tasks (testing helper).
  #[must_use]
  pub fn pending_tasks(&self) -> usize {
    self.queue.lock().len()
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
  fn execute(&self, dispatcher: DispatchSharedGeneric<TB>) -> Result<(), DispatchError> {
    self.queue.lock().push_back(dispatcher);
    Ok(())
  }
}
