//! Minimal future primitive used by the ask pattern.

#[cfg(test)]
mod tests;

use core::task::Waker;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::ArcShared,
};

use crate::core::futures::actor_future_listener::ActorFutureListener;

/// Represents a future that resolves with a message.
///
/// This type no longer uses interior mutability. Methods that modify state
/// require `&mut self`. Use [`ActorFutureShared`] for shared ownership with
/// external mutex synchronization.
pub struct ActorFuture<T, TB: RuntimeToolbox = NoStdToolbox>
where
  T: Send + 'static, {
  value:   Option<T>,
  waker:   Option<Waker>,
  _marker: core::marker::PhantomData<TB>,
}

/// Shared wrapper for [`ActorFuture`] with external mutex synchronization.
///
/// This type provides thread-safe shared access to an `ActorFuture` by wrapping
/// it in `ArcShared<ToolboxMutex<...>>`. Callers must lock the mutex before
/// calling mutable methods.
pub type ActorFutureShared<T, TB> = ArcShared<ToolboxMutex<ActorFuture<T, TB>, TB>>;

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

  /// Creates a new shared future wrapped in `ArcShared<ToolboxMutex<...>>`.
  #[must_use]
  pub fn new_shared() -> ActorFutureShared<T, TB> {
    ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(Self::new()))
  }

  /// Completes the future with a value and returns the waker if registered.
  ///
  /// Subsequent calls return `None`.
  ///
  /// # Important
  ///
  /// The caller **must** wake the returned waker after releasing the lock to
  /// avoid deadlock. Use [`complete_and_wake`](Self::complete_and_wake) for a
  /// safe wrapper when working with [`ActorFutureShared`].
  pub fn complete(&mut self, value: T) -> Option<Waker> {
    if self.value.is_some() {
      return None;
    }
    self.value = Some(value);
    self.waker.take()
  }

  /// Completes a shared future and wakes the waker safely.
  ///
  /// This method acquires the lock, sets the value, releases the lock, and
  /// then wakes the waker. This ordering prevents deadlock when the waker
  /// immediately polls the future.
  pub fn complete_and_wake(shared: &ActorFutureShared<T, TB>, value: T) {
    use fraktor_utils_rs::core::sync::sync_mutex_like::SyncMutexLike;
    let waker = shared.lock().complete(value);
    if let Some(w) = waker {
      w.wake();
    }
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

  /// Returns a lightweight adapter implementing [`Future`].
  ///
  /// The listener holds a shared reference to the future and locks the mutex
  /// on each poll.
  #[must_use]
  pub const fn listener(this: ActorFutureShared<T, TB>) -> ActorFutureListener<T, TB> {
    ActorFutureListener::new(this)
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
