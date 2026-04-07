#[cfg(test)]
mod tests;

/// Thin wrapper around [`spin::RwLock`].
pub struct SpinSyncRwLock<T>(spin::RwLock<T>);

unsafe impl<T: Send> Send for SpinSyncRwLock<T> {}
unsafe impl<T: Send + Sync> Sync for SpinSyncRwLock<T> {}

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

  /// Acquires a shared read guard.
  pub fn read(&self) -> spin::RwLockReadGuard<'_, T> {
    self.0.read()
  }

  /// Acquires an exclusive write guard.
  pub fn write(&self) -> spin::RwLockWriteGuard<'_, T> {
    self.0.write()
  }
}
