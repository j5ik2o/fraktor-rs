use core::ops::{Deref, DerefMut};

/// Runtime-selectable rwlock driver contract.
pub trait RwLockDriver<T>: Sized {
  /// Shared read guard.
  type ReadGuard<'a>: Deref<Target = T>
  where
    Self: 'a,
    T: 'a;

  /// Exclusive write guard.
  type WriteGuard<'a>: Deref<Target = T> + DerefMut
  where
    Self: 'a,
    T: 'a;

  /// Creates a new driver instance containing `value`.
  fn new(value: T) -> Self;

  /// Acquires a shared read guard.
  fn read(&self) -> Self::ReadGuard<'_>;

  /// Acquires an exclusive write guard.
  fn write(&self) -> Self::WriteGuard<'_>;

  /// Consumes the driver and returns the protected value.
  fn into_inner(self) -> T;
}
