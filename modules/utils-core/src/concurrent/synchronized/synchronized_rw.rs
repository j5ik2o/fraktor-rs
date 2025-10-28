use core::marker::PhantomData;

use super::{guard_handle::GuardHandle, synchronized_rw_backend::SynchronizedRwBackend};

/// Async read/write synchronization primitive providing backend abstraction.
#[derive(Debug)]
pub struct SynchronizedRw<B, T: ?Sized>
where
  B: SynchronizedRwBackend<T>, {
  backend: B,
  _marker: PhantomData<T>,
}

impl<B, T> SynchronizedRw<B, T>
where
  T: ?Sized,
  B: SynchronizedRwBackend<T>,
{
  /// Creates a new `SynchronizedRw` with the specified value.
  #[must_use]
  pub fn new(value: T) -> Self
  where
    T: Sized, {
    Self { backend: B::new(value), _marker: PhantomData }
  }

  /// Creates a `SynchronizedRw` from an existing backend.
  #[must_use]
  pub const fn from_backend(backend: B) -> Self {
    Self { backend, _marker: PhantomData }
  }

  /// Gets a reference to the internal backend.
  #[must_use]
  pub const fn backend(&self) -> &B {
    &self.backend
  }

  /// Acquires a read lock and executes the specified function.
  pub async fn read<R>(&self, f: impl FnOnce(&B::ReadGuard<'_>) -> R) -> R {
    let guard = self.backend.read().await;
    f(&guard)
  }

  /// Acquires a write lock and executes the specified function.
  pub async fn write<R>(&self, f: impl FnOnce(&mut B::WriteGuard<'_>) -> R) -> R {
    let mut guard = self.backend.write().await;
    f(&mut guard)
  }

  /// Acquires a read lock and returns a guard handle.
  pub async fn read_guard(&self) -> GuardHandle<B::ReadGuard<'_>> {
    let guard = self.backend.read().await;
    GuardHandle::new(guard)
  }

  /// Acquires a write lock and returns a guard handle.
  pub async fn write_guard(&self) -> GuardHandle<B::WriteGuard<'_>> {
    let guard = self.backend.write().await;
    GuardHandle::new(guard)
  }
}

impl<B, T> Default for SynchronizedRw<B, T>
where
  T: Default,
  B: SynchronizedRwBackend<T>,
{
  fn default() -> Self {
    Self::new(T::default())
  }
}

impl<B, T> From<T> for SynchronizedRw<B, T>
where
  T: Sized,
  B: SynchronizedRwBackend<T>,
{
  fn from(value: T) -> Self {
    Self::new(value)
  }
}
