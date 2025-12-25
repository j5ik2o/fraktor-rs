//! Shared wrapper for actor future.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::ActorFuture;

/// Shared wrapper for [`ActorFuture`] with external mutex synchronization.
///
/// This type provides thread-safe shared access to an `ActorFuture` by wrapping
/// it in `ArcShared<ToolboxMutex<...>>`. This is a thin wrapper that delegates
/// all operations to the inner type by acquiring a lock and calling the
/// corresponding method on [`ActorFuture`].
pub struct ActorFutureSharedGeneric<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static, {
  inner: ArcShared<ToolboxMutex<ActorFuture<T, TB>, TB>>,
}

impl<T, TB> Clone for ActorFutureSharedGeneric<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<T, TB> SharedAccess<ActorFuture<T, TB>> for ActorFutureSharedGeneric<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn with_read<R>(&self, f: impl FnOnce(&ActorFuture<T, TB>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut ActorFuture<T, TB>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}

impl<T, TB> ActorFutureSharedGeneric<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new shared future wrapped in `ArcShared<ToolboxMutex<...>>`.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(ActorFuture::new())) }
  }
}

impl<T, TB> Default for ActorFutureSharedGeneric<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn default() -> Self {
    Self::new()
  }
}

/// Type alias with the default `NoStdToolbox`.
pub type ActorFutureShared<T> = ActorFutureSharedGeneric<T, NoStdToolbox>;
