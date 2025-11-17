use alloc::boxed::Box;
use core::ops::{Deref, DerefMut};

use async_trait::async_trait;

/// Backend trait for async read/write lock primitives.
#[async_trait(?Send)]
pub trait SynchronizedRwBackend<T: ?Sized> {
  /// Guard type returned when read lock is acquired.
  type ReadGuard<'a>: Deref<Target = T> + 'a
  where
    Self: 'a;

  /// Guard type returned when write lock is acquired.
  type WriteGuard<'a>: Deref<Target = T> + DerefMut + 'a
  where
    Self: 'a;

  /// Creates a new backend with the specified value.
  fn new(value: T) -> Self
  where
    T: Sized;

  /// Acquires a read lock.
  async fn read(&self) -> Self::ReadGuard<'_>;

  /// Acquires a write lock.
  async fn write(&self) -> Self::WriteGuard<'_>;
}
