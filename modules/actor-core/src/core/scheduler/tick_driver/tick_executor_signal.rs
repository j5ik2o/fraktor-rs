//! Executor wakeup signal shared between drivers and scheduler tasks.

#[cfg(test)]
mod tests;

use core::sync::atomic::{AtomicBool, Ordering};

use fraktor_utils_rs::core::sync::ArcShared;
use futures::task::AtomicWaker;

use super::tick_executor_signal_future::TickExecutorSignalFuture;

/// Notifies scheduler executors when new ticks arrive.
#[derive(Clone)]
pub struct TickExecutorSignal {
  state: ArcShared<TickExecutorSignalState>,
}

impl TickExecutorSignal {
  /// Creates a new signal instance.
  #[must_use]
  pub fn new() -> Self {
    Self { state: ArcShared::new(TickExecutorSignalState::new()) }
  }

  /// Notifies waiting executors.
  pub fn notify(&self) {
    self.state.mark_pending();
  }

  /// Arms the signal for no_std polling and returns whether work is pending.
  #[must_use]
  pub fn arm(&self) -> bool {
    self.state.take_pending()
  }

  /// Returns a future that resolves once work is available.
  pub fn wait_async(&self) -> impl core::future::Future<Output = ()> + '_ {
    TickExecutorSignalFuture { signal: self }
  }

  pub(crate) fn register_waker(&self, waker: &core::task::Waker) {
    self.state.register_waker(waker);
  }
}

impl Default for TickExecutorSignal {
  fn default() -> Self {
    Self::new()
  }
}

struct TickExecutorSignalState {
  pending: AtomicBool,
  waker:   AtomicWaker,
}

impl TickExecutorSignalState {
  const fn new() -> Self {
    Self { pending: AtomicBool::new(false), waker: AtomicWaker::new() }
  }

  fn mark_pending(&self) {
    self.pending.store(true, Ordering::Release);
    self.waker.wake();
  }

  fn take_pending(&self) -> bool {
    self.pending.swap(false, Ordering::AcqRel)
  }

  fn register_waker(&self, waker: &core::task::Waker) {
    self.waker.register(waker);
  }
}
