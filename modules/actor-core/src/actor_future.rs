//! Minimal future primitive used by the ask pattern.

use core::task::Waker;

use cellactor_utils_core_rs::sync::{sync_mutex_like::SyncMutexLike, SyncMutexFamily};

use crate::{actor_future_listener::ActorFutureListener, NoStdToolbox, RuntimeToolbox, ToolboxMutex};

/// Represents a future that resolves with a message.
pub struct ActorFuture<T, TB: RuntimeToolbox = NoStdToolbox>
where
  T: Send + 'static, {
  value: ToolboxMutex<Option<T>, TB>,
  waker: ToolboxMutex<Option<Waker>, TB>,
}

impl<T, TB> ActorFuture<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
{
  /// Creates a new future in the pending state.
  #[must_use]
  pub fn new() -> Self {
    Self {
      value: <TB::MutexFamily as SyncMutexFamily>::create(None),
      waker: <TB::MutexFamily as SyncMutexFamily>::create(None),
    }
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
  #[must_use]
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
  pub fn listener(&self) -> ActorFutureListener<'_, T, TB> {
    ActorFutureListener::new(self)
  }

  pub(crate) fn register_waker(&self, waker: &Waker) {
    *self.waker.lock() = Some(waker.clone());
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

// SAFETY: `ActorFuture` delegates synchronization to the mutex implementation supplied by the
// toolbox. As long as the stored value is `Send`, the mutex guarantees sound interior mutability.
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

#[cfg(test)]
mod tests;
