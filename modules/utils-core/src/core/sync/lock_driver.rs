use core::ops::{Deref, DerefMut};

/// Driver contract for runtime-selectable mutex implementations.
pub trait LockDriver<T>: Sized {
  /// Guard returned by [`Self::lock`].
  type Guard<'a>: Deref<Target = T> + DerefMut
  where
    Self: 'a,
    T: 'a;

  /// Creates a new driver instance containing `value`.
  fn new(value: T) -> Self;

  /// Locks the driver and returns a mutable guard.
  fn lock(&self) -> Self::Guard<'_>;

  /// Consumes the driver and returns the protected value.
  fn into_inner(self) -> T;
}
