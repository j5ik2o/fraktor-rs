//! Minimal future primitive used by the ask pattern.

#[cfg(test)]
#[path = "actor_future_test.rs"]
mod tests;

use core::{marker::PhantomData, task::Waker};

/// Represents a future that resolves with a message.
///
/// This type no longer uses interior mutability. Methods that modify state
/// require `&mut self`. Use [`ActorFutureShared`](super::ActorFutureShared) for
/// shared ownership with external mutex synchronization.
pub struct ActorFuture<T>
where
  T: Send + 'static, {
  value:   Option<T>,
  waker:   Option<Waker>,
  _marker: PhantomData<()>,
}

impl<T> ActorFuture<T>
where
  T: Send + 'static,
{
  /// Creates a new future in the pending state.
  #[must_use]
  pub const fn new() -> Self {
    Self { value: None, waker: None, _marker: PhantomData }
  }

  /// Completes the future with a value and returns the waker if registered.
  ///
  /// Subsequent calls return `None`.
  ///
  /// # Important
  ///
  /// The caller **must** wake the returned waker after releasing the lock to
  /// avoid deadlock. See [`ActorFutureShared`](super::ActorFutureShared) for a
  /// safe wrapper when working with shared futures.
  pub fn complete(&mut self, value: T) -> Option<Waker> {
    if self.value.is_some() {
      return None;
    }
    self.value = Some(value);
    self.waker.take()
  }

  /// Attempts to take the result if available.
  #[must_use]
  pub const fn try_take(&mut self) -> Option<T> {
    self.value.take()
  }

  /// Returns whether the future has resolved.
  #[must_use]
  pub const fn is_ready(&self) -> bool {
    self.value.is_some()
  }

  /// Registers a waker to be notified when the future completes.
  pub fn register_waker(&mut self, waker: &Waker) {
    self.waker = Some(waker.clone());
  }
}

impl<T> Default for ActorFuture<T>
where
  T: Send + 'static,
{
  fn default() -> Self {
    Self::new()
  }
}

// SAFETY: `ActorFuture` fields are only accessed through `&mut self` methods.
// When wrapped in `SpinSyncMutex`, the mutex provides synchronization.
// The stored value must be `Send` to allow transfer between threads.
unsafe impl<T> Send for ActorFuture<T> where T: Send + 'static {}

unsafe impl<T> Sync for ActorFuture<T> where T: Send + 'static {}
