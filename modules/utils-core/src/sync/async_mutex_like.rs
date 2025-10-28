use alloc::boxed::Box;
use core::ops::{Deref, DerefMut};

use async_trait::async_trait;
mod spin_async_mutex;
pub use spin_async_mutex::*;

use crate::v2::sync::SharedError;

/// Async-aware mutex abstraction.
#[async_trait(?Send)]
pub trait AsyncMutexLike<T> {
  /// Guard type returned by [`AsyncMutexLike::lock`].
  type Guard<'a>: Deref<Target = T> + DerefMut
  where
    Self: 'a,
    T: 'a;

  /// Creates a new mutex instance wrapping the provided value.
  fn new(value: T) -> Self;

  /// Consumes the mutex and returns the inner value.
  fn into_inner(self) -> T;

  /// Asynchronously locks the mutex and yields a guard to the protected value.
  async fn lock(&self) -> Result<Self::Guard<'_>, SharedError>;
}

/// Convenience alias for guards produced by [`AsyncMutexLike`].
pub type AsyncMutexLikeGuard<'a, M, T> = <M as AsyncMutexLike<T>>::Guard<'a>;
