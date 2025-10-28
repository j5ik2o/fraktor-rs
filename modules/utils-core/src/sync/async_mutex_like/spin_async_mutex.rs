use alloc::boxed::Box;
use core::marker::PhantomData;

use async_trait::async_trait;

#[cfg(feature = "interrupt-cortex-m")]
use crate::sync::interrupt::CriticalSectionInterruptPolicy;
use crate::{
  sync::{
    async_mutex_like::AsyncMutexLike,
    interrupt::{InterruptContextPolicy, NeverInterruptPolicy},
  },
  v2::sync::SharedError,
};

type SpinGuard<'a, T> = spin::MutexGuard<'a, T>;

/// Thin wrapper around [`spin::Mutex`] implementing [`AsyncMutexLike`].
#[allow(dead_code)]
pub struct SpinAsyncMutex<T, P = NeverInterruptPolicy>
where
  P: InterruptContextPolicy, {
  inner:   spin::Mutex<T>,
  _policy: PhantomData<P>,
}

#[allow(dead_code)]
impl<T, P> SpinAsyncMutex<T, P>
where
  P: InterruptContextPolicy,
{
  /// Creates a new spinlock-protected value.
  #[must_use]
  pub const fn new(value: T) -> Self {
    Self { inner: spin::Mutex::new(value), _policy: PhantomData }
  }

  /// Returns a reference to the inner spin mutex.
  #[must_use]
  pub const fn as_inner(&self) -> &spin::Mutex<T> {
    &self.inner
  }

  /// Consumes the wrapper and returns the underlying value.
  pub fn into_inner(self) -> T {
    self.inner.into_inner()
  }

  /// Locks the mutex and returns a guard to the protected value.
  pub fn lock(&self) -> SpinGuard<'_, T> {
    self.inner.lock()
  }
}

#[async_trait(?Send)]
impl<T, P> AsyncMutexLike<T> for SpinAsyncMutex<T, P>
where
  P: InterruptContextPolicy + Send + Sync,
{
  type Guard<'a>
    = SpinGuard<'a, T>
  where
    T: 'a,
    P: 'a;

  fn new(value: T) -> Self {
    SpinAsyncMutex::new(value)
  }

  fn into_inner(self) -> T {
    SpinAsyncMutex::into_inner(self)
  }

  async fn lock(&self) -> Result<Self::Guard<'_>, SharedError> {
    P::check_blocking_allowed()?;
    Ok(SpinAsyncMutex::lock(self))
  }
}

/// Default spin-based async mutex that never flags interrupt contexts.
pub type SpinAsyncMutexDefault<T> = SpinAsyncMutex<T, NeverInterruptPolicy>;

/// Spin-based async mutex tailored for Cortex-M targets; operations are only allowed in thread
/// mode.
#[cfg(feature = "interrupt-cortex-m")]
pub type SpinAsyncMutexCritical<T> = SpinAsyncMutex<T, CriticalSectionInterruptPolicy>;
