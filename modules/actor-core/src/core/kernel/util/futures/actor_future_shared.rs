//! Shared wrapper for actor future.

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock};

use super::ActorFuture;

/// Shared wrapper for [`ActorFuture`] with external mutex synchronization.
///
/// This type provides thread-safe shared access to an `ActorFuture` by wrapping
/// it in `SharedLock<...>`. This is a thin wrapper that delegates
/// all operations to the inner type by acquiring a lock and calling the
/// corresponding method on [`ActorFuture`].
pub struct ActorFutureShared<T>
where
  T: Send + 'static, {
  inner: SharedLock<ActorFuture<T>>,
}

impl<T> Clone for ActorFutureShared<T>
where
  T: Send + 'static,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<T> SharedAccess<ActorFuture<T>> for ActorFutureShared<T>
where
  T: Send + 'static,
{
  fn with_read<R>(&self, f: impl FnOnce(&ActorFuture<T>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut ActorFuture<T>) -> R) -> R {
    self.inner.with_write(f)
  }
}

impl<T> ActorFutureShared<T>
where
  T: Send + 'static,
{
  /// Creates a shared wrapper from an existing shared lock.
  #[must_use]
  pub const fn from_shared_lock(inner: SharedLock<ActorFuture<T>>) -> Self {
    Self { inner }
  }
}
