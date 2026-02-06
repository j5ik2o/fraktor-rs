//! Minimal future primitive used by the ask pattern.

#[cfg(test)]
mod tests;

use core::task::Waker;

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

/// Represents a future that resolves with a message.
///
/// This type no longer uses interior mutability. Methods that modify state
/// require `&mut self`. Use [`ActorFutureSharedGeneric`](super::ActorFutureSharedGeneric) for
/// shared ownership with external mutex synchronization.
pub struct ActorFuture<T, TB: RuntimeToolbox = NoStdToolbox>
where
  T: Send + 'static, {
  value:   Option<T>,
  waker:   Option<Waker>,
  _marker: core::marker::PhantomData<TB>,
}

impl<T, TB> ActorFuture<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
{
  /// Creates a new future in the pending state.
  #[must_use]
  pub const fn new() -> Self {
    Self { value: None, waker: None, _marker: core::marker::PhantomData }
  }

  /// Completes the future with a value and returns the waker if registered.
  ///
  /// Subsequent calls return `None`.
  ///
  /// # Important
  ///
  /// The caller **must** wake the returned waker after releasing the lock to
  /// avoid deadlock. See [`ActorFutureSharedGeneric`](super::ActorFutureSharedGeneric) for a
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

impl<T, TB> Default for ActorFuture<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
{
  fn default() -> Self {
    Self::new()
  }
}

// SAFETY: `ActorFuture` fields are only accessed through `&mut self` methods.
// When wrapped in `ToolboxMutex`, the mutex provides synchronization.
// The stored value must be `Send` to allow transfer between threads.
unsafe impl<T, TB> Send for ActorFuture<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
{
}

unsafe impl<T, TB> Sync for ActorFuture<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
{
}
