//! Shared wrapper for actor future.

use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeMutex,
  sync::{ArcShared, SharedAccess},
};

use super::ActorFuture;

/// Shared wrapper for [`ActorFuture`] with external mutex synchronization.
///
/// This type provides thread-safe shared access to an `ActorFuture` by wrapping
/// it in `ArcShared<RuntimeMutex<...>>`. This is a thin wrapper that delegates
/// all operations to the inner type by acquiring a lock and calling the
/// corresponding method on [`ActorFuture`].
pub struct ActorFutureShared<T>
where
  T: Send + 'static, {
  inner: ArcShared<RuntimeMutex<ActorFuture<T>>>,
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
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut ActorFuture<T>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}

impl<T> ActorFutureShared<T>
where
  T: Send + 'static,
{
  /// Creates a new shared future wrapped in `ArcShared<RuntimeMutex<...>>`.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(ActorFuture::new())) }
  }
}

impl<T> Default for ActorFutureShared<T>
where
  T: Send + 'static,
{
  fn default() -> Self {
    Self::new()
  }
}
