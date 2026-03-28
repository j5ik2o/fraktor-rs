use alloc::collections::VecDeque;

use super::{dispatch_error::DispatchError, dispatch_executor::DispatchExecutor, dispatch_shared::DispatchShared};

/// Executor that queues dispatcher batches until `tick` is invoked.
pub struct TickExecutor {
  queue: VecDeque<DispatchShared>,
}

impl TickExecutor {
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

impl Default for TickExecutor {
  fn default() -> Self {
    Self::new()
  }
}

impl DispatchExecutor for TickExecutor {
  fn execute(&mut self, dispatcher: DispatchShared) -> Result<(), DispatchError> {
    self.queue.push_back(dispatcher);
    Ok(())
  }
}
