//! [`Executor`] backed by an Embassy-ready bounded queue.

#[cfg(test)]
#[path = "embassy_executor_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::cell::Cell;

use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use fraktor_actor_core_kernel_rs::dispatch::dispatcher::{ExecuteError, Executor};

use super::embassy_executor_shared::EmbassyExecutorShared;

/// Executor that enqueues actor mailbox work for an Embassy worker task.
pub struct EmbassyExecutor<const N: usize> {
  shared:    EmbassyExecutorShared<N>,
  accepting: Mutex<CriticalSectionRawMutex, Cell<bool>>,
}

impl<const N: usize> EmbassyExecutor<N> {
  pub(crate) const fn new(shared: EmbassyExecutorShared<N>) -> Self {
    Self { shared, accepting: Mutex::new(Cell::new(true)) }
  }
}

impl<const N: usize> Executor for EmbassyExecutor<N> {
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
    self.accepting.lock(|accepting| {
      if !accepting.get() {
        return Err(ExecuteError::Shutdown);
      }
      self.shared.try_enqueue(task)
    })?;
    self.shared.signal_ready();
    Ok(())
  }

  fn shutdown(&mut self) {
    self.accepting.lock(|accepting| {
      accepting.set(false);
    });
    self.shared.signal_ready();
  }
}
