//! [`Executor`] backed by an Embassy-ready bounded queue.

#[cfg(test)]
#[path = "embassy_executor_test.rs"]
mod tests;

use alloc::{boxed::Box, sync::Arc};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, signal::Signal};
use fraktor_actor_core_kernel_rs::dispatch::dispatcher::{ExecuteError, Executor};

pub(crate) type EmbassyTask = Box<dyn FnOnce() + Send + 'static>;

/// Executor that enqueues actor mailbox work for an Embassy worker task.
pub struct EmbassyExecutor<const N: usize> {
  queue:     Arc<Channel<CriticalSectionRawMutex, EmbassyTask, N>>,
  signal:    Arc<Signal<CriticalSectionRawMutex, ()>>,
  accepting: bool,
}

impl<const N: usize> EmbassyExecutor<N> {
  pub(crate) fn new() -> Self {
    Self { queue: Arc::new(Channel::new()), signal: Arc::new(Signal::new()), accepting: true }
  }

  pub(crate) fn clone_for_submission(&self) -> Self {
    Self { queue: self.queue.clone(), signal: self.signal.clone(), accepting: true }
  }

  pub(crate) fn ready_signal(&self) -> Arc<Signal<CriticalSectionRawMutex, ()>> {
    self.signal.clone()
  }

  pub(crate) fn pop_ready(&mut self) -> Option<EmbassyTask> {
    self.queue.try_receive().ok()
  }
}

impl<const N: usize> Executor for EmbassyExecutor<N> {
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
    if !self.accepting {
      return Err(ExecuteError::Shutdown);
    }
    self.queue.try_send(task).map_err(|_| ExecuteError::Rejected)?;
    self.signal.signal(());
    Ok(())
  }

  fn shutdown(&mut self) {
    self.accepting = false;
    self.signal.signal(());
  }
}
