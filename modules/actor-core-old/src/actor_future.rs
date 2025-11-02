//! Minimal future primitive for ask pattern handling.

use core::task::Waker;

use crate::{ActorRuntimeMutex, actor_future_listener::ActorFutureListener};

#[cfg(test)]
mod tests;

/// Minimal future primitive used by the ask pattern.
pub struct ActorFuture<T> {
  value: ActorRuntimeMutex<Option<T>>,
  waker: ActorRuntimeMutex<Option<Waker>>,
}

impl<T> ActorFuture<T> {
  /// Creates a new future in the pending state.
  #[must_use]
  pub const fn new() -> Self {
    Self { value: ActorRuntimeMutex::new(None), waker: ActorRuntimeMutex::new(None) }
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
  pub const fn listener(&self) -> ActorFutureListener<'_, T> {
    ActorFutureListener::new(self)
  }

  pub(crate) fn register_waker(&self, waker: &Waker) {
    *self.waker.lock() = Some(waker.clone());
  }
}

impl<T> Default for ActorFuture<T> {
  fn default() -> Self {
    Self::new()
  }
}
