use crate::core::sync::sync_rwlock_like::SyncRwLockLike;

#[cfg(test)]
mod tests;

/// Thin wrapper around [`spin::RwLock`] implementing [`SyncRwLockLike`].
pub struct SpinSyncRwLock<T>(spin::RwLock<T>);

unsafe impl<T: Send> Send for SpinSyncRwLock<T> {}
unsafe impl<T: Send> Sync for SpinSyncRwLock<T> {}

impl<T> SpinSyncRwLock<T> {
  /// Creates a new spin-based read-write lock.
  #[must_use]
  pub const fn new(value: T) -> Self {
    Self(spin::RwLock::new(value))
  }

  /// Returns a reference to the inner lock.
  #[must_use]
  pub const fn as_inner(&self) -> &spin::RwLock<T> {
    &self.0
  }

  /// Consumes the lock and returns the inner value.
  pub fn into_inner(self) -> T {
    self.0.into_inner()
  }
}

impl<T> SyncRwLockLike<T> for SpinSyncRwLock<T> {
  type ReadGuard<'a>
    = spin::RwLockReadGuard<'a, T>
  where
    T: 'a;
  type WriteGuard<'a>
    = spin::RwLockWriteGuard<'a, T>
  where
    T: 'a;

  fn new(value: T) -> Self {
    SpinSyncRwLock::new(value)
  }

  fn into_inner(self) -> T {
    SpinSyncRwLock::into_inner(self)
  }

  fn read(&self) -> Self::ReadGuard<'_> {
    self.0.read()
  }

  fn write(&self) -> Self::WriteGuard<'_> {
    self.0.write()
  }
}
