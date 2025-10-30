//! Minimal future primitive for ask pattern handling.

use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll, Waker},
};

use cellactor_utils_core_rs::sync::sync_mutex_like::SpinSyncMutex;

/// Minimal future primitive used by the ask pattern.
pub struct ActorFuture<T> {
  value: SpinSyncMutex<Option<T>>,
  waker: SpinSyncMutex<Option<Waker>>,
}

impl<T> ActorFuture<T> {
  /// Creates a new future in the pending state.
  #[must_use]
  pub const fn new() -> Self {
    Self { value: SpinSyncMutex::new(None), waker: SpinSyncMutex::new(None) }
  }

  /// Completes the future with a value. Subsequent calls are ignored.
  pub fn complete(&self, value: T) {
    let mut slot = self.value.lock();
    if slot.is_some() {
      return;
    }
    *slot = Some(value);
    drop(slot);

    if let Some(waker) = self.waker.lock().take() {
      waker.wake();
    }
  }

  /// Attempts to take the result if available.
  pub fn try_take(&self) -> Option<T> {
    self.value.lock().take()
  }

  /// Returns whether the future has resolved.
  #[must_use]
  pub fn is_ready(&self) -> bool {
    self.value.lock().is_some()
  }

  /// Returns a lightweight adapter implementing [`Future`].
  #[must_use]
  pub fn listener(&self) -> ActorFutureListener<'_, T> {
    ActorFutureListener::new(self)
  }

  fn register_waker(&self, waker: &Waker) {
    *self.waker.lock() = Some(waker.clone());
  }
}

impl<T> Default for ActorFuture<T> {
  fn default() -> Self {
    Self::new()
  }
}

/// Future adapter that polls the underlying [`ActorFuture`].
pub struct ActorFutureListener<'a, T> {
  future: &'a ActorFuture<T>,
}

impl<'a, T> ActorFutureListener<'a, T> {
  const fn new(future: &'a ActorFuture<T>) -> Self {
    Self { future }
  }
}

impl<'a, T> Future for ActorFutureListener<'a, T> {
  type Output = T;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    if let Some(value) = self.future.try_take() {
      Poll::Ready(value)
    } else {
      self.future.register_waker(cx.waker());
      Poll::Pending
    }
  }
}

impl<'a, T> Unpin for ActorFutureListener<'a, T> {}

#[cfg(test)]
mod tests;
