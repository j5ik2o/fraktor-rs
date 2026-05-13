//! Shared queue state for Embassy executors.

use alloc::{boxed::Box, sync::Arc};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, signal::Signal};
use fraktor_actor_core_kernel_rs::dispatch::dispatcher::ExecuteError;

pub(crate) type EmbassyTask = Box<dyn FnOnce() + Send + 'static>;

pub(crate) struct EmbassyExecutorShared<const N: usize> {
  queue:  Arc<Channel<CriticalSectionRawMutex, EmbassyTask, N>>,
  signal: Arc<Signal<CriticalSectionRawMutex, ()>>,
}

impl<const N: usize> EmbassyExecutorShared<N> {
  pub(crate) fn new() -> Self {
    Self { queue: Arc::new(Channel::new()), signal: Arc::new(Signal::new()) }
  }

  pub(crate) fn try_enqueue(&self, task: EmbassyTask) -> Result<(), ExecuteError> {
    self.queue.try_send(task).map_err(|_| ExecuteError::Rejected)
  }

  pub(crate) fn signal_ready(&self) {
    self.signal.signal(());
  }

  pub(crate) async fn wait_ready(&self) {
    self.signal.wait().await;
  }

  pub(crate) fn drain_ready(&self) -> usize {
    let mut drained = 0;
    while let Ok(task) = self.queue.try_receive() {
      task();
      drained += 1;
    }
    drained
  }
}

impl<const N: usize> Clone for EmbassyExecutorShared<N> {
  fn clone(&self) -> Self {
    Self { queue: self.queue.clone(), signal: self.signal.clone() }
  }
}
