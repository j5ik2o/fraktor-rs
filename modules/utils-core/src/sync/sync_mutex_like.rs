use core::ops::{Deref, DerefMut};

mod spin_sync_mutex;

pub use spin_sync_mutex::*;

/// Generic mutex abstraction for runtime-agnostic code.
pub trait SyncMutexLike<T> {
  /// Guard type returned by [`SyncMutexLike::lock`].
  type Guard<'a>: Deref<Target = T> + DerefMut
  where
    Self: 'a,
    T: 'a;

  /// Creates a new mutex instance wrapping the provided value.
  fn new(value: T) -> Self;

  /// Consumes the mutex and returns the inner value.
  fn into_inner(self) -> T;

  /// Locks the mutex and returns a guard to the protected value.
  fn lock(&self) -> Self::Guard<'_>;
}

/// Convenience alias for guards produced by [`SyncMutexLike`].
pub type SyncMutexLikeGuard<'a, M, T> = <M as SyncMutexLike<T>>::Guard<'a>;
