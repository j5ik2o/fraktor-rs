//! Executor wakeup signal shared between drivers and scheduler tasks.

#[cfg(test)]
mod tests;

use core::sync::atomic::{AtomicBool, Ordering};

use fraktor_utils_core_rs::sync::ArcShared;
use futures::task::AtomicWaker;

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
  pub fn arm(&self) -> bool {
    self.state.take_pending()
  }

  /// Returns a future that resolves once work is available.
  pub fn wait_async(&self) -> TickExecutorSignalFuture<'_> {
    TickExecutorSignalFuture { signal: self }
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

/// Future returned by [`TickExecutorSignal::wait_async`].
pub struct TickExecutorSignalFuture<'a> {
  signal: &'a TickExecutorSignal,
}

impl core::future::Future for TickExecutorSignalFuture<'_> {
  type Output = ();

  fn poll(self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
    if self.signal.arm() {
      return core::task::Poll::Ready(());
    }
    self.signal.state.register_waker(cx.waker());
    if self.signal.arm() { core::task::Poll::Ready(()) } else { core::task::Poll::Pending }
  }
}
