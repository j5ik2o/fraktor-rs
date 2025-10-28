use crate::sync::sync_mutex_like::SyncMutexLike;

/// Thin wrapper around [`spin::Mutex`] implementing [`SyncMutexLike`].
pub struct SpinSyncMutex<T>(spin::Mutex<T>);

impl<T> SpinSyncMutex<T> {
  /// Creates a new spinlock-protected value.
  #[must_use]
  pub const fn new(value: T) -> Self {
    Self(spin::Mutex::new(value))
  }

  /// Returns a reference to the inner spin mutex.
  #[must_use]
  pub const fn as_inner(&self) -> &spin::Mutex<T> {
    &self.0
  }

  /// Consumes the wrapper and returns the underlying value.
  pub fn into_inner(self) -> T {
    self.0.into_inner()
  }

  /// Locks the mutex and returns a guard to the protected value.
  pub fn lock(&self) -> spin::MutexGuard<'_, T> {
    self.0.lock()
  }
}

impl<T> SyncMutexLike<T> for SpinSyncMutex<T> {
  type Guard<'a>
    = spin::MutexGuard<'a, T>
  where
    T: 'a;

  fn new(value: T) -> Self {
    SpinSyncMutex::new(value)
  }

  fn into_inner(self) -> T {
    SpinSyncMutex::into_inner(self)
  }

  fn lock(&self) -> Self::Guard<'_> {
    SpinSyncMutex::lock(self)
  }
}
