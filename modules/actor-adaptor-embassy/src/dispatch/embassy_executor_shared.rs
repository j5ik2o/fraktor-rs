//! Shared queue state for Embassy executors.

use alloc::{boxed::Box, sync::Arc};
use core::sync::atomic::{AtomicBool, Ordering};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, signal::Signal};
use fraktor_actor_core_kernel_rs::dispatch::dispatcher::ExecuteError;

pub(crate) type EmbassyTask = Box<dyn FnOnce() + Send + 'static>;

pub(crate) struct EmbassyExecutorShared<const N: usize> {
  queue:     Arc<Channel<CriticalSectionRawMutex, EmbassyTask, N>>,
  signal:    Arc<Signal<CriticalSectionRawMutex, ()>>,
  accepting: Arc<AtomicBool>,
}

impl<const N: usize> EmbassyExecutorShared<N> {
  pub(crate) fn new() -> Self {
    Self {
      queue:     Arc::new(Channel::new()),
      signal:    Arc::new(Signal::new()),
      accepting: Arc::new(AtomicBool::new(true)),
    }
  }

  pub(crate) fn enqueue(&self, task: EmbassyTask) -> Result<(), ExecuteError> {
    if !self.accepting.load(Ordering::Acquire) {
      return Err(ExecuteError::Shutdown);
    }
    self.queue.try_send(task).map_err(|_| ExecuteError::Rejected)?;
    self.signal.signal(());
    Ok(())
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

  pub(crate) fn shutdown(&self) {
    self.accepting.store(false, Ordering::Release);
    self.signal.signal(());
  }
}

impl<const N: usize> Clone for EmbassyExecutorShared<N> {
  fn clone(&self) -> Self {
    Self { queue: self.queue.clone(), signal: self.signal.clone(), accepting: self.accepting.clone() }
  }
}
