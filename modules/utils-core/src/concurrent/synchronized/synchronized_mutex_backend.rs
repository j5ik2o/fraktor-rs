use alloc::boxed::Box;
use core::ops::{Deref, DerefMut};

use async_trait::async_trait;

use crate::sync::SharedError;

/// Backend trait for async mutex-like primitives.
#[async_trait(?Send)]
pub trait SynchronizedMutexBackend<T: ?Sized> {
  /// Guard type returned when lock is acquired.
  type Guard<'a>: Deref<Target = T> + DerefMut + 'a
  where
    Self: 'a;

  /// Creates a new backend with the specified value.
  fn new(value: T) -> Self
  where
    T: Sized;

  /// Locks the mutex and obtains a guard.
  async fn lock(&self) -> Result<Self::Guard<'_>, SharedError>;
}
