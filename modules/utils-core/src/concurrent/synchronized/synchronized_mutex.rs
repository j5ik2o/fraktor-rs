use core::marker::PhantomData;

use super::{guard_handle::GuardHandle, synchronized_mutex_backend::SynchronizedMutexBackend};

/// Async synchronization primitive providing exclusive access.
#[derive(Debug)]
pub struct Synchronized<B, T: ?Sized>
where
  B: SynchronizedMutexBackend<T>, {
  backend: B,
  _marker: PhantomData<T>,
}

impl<B, T> Synchronized<B, T>
where
  T: ?Sized,
  B: SynchronizedMutexBackend<T>,
{
  /// Creates a new `Synchronized` with the specified value.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn new(value: T) -> Self
  where
    T: Sized, {
    Self { backend: B::new(value), _marker: PhantomData }
  }

  /// Creates a `Synchronized` from an existing backend.
  #[must_use]
  pub const fn from_backend(backend: B) -> Self {
    Self { backend, _marker: PhantomData }
  }

  /// Gets a reference to the internal backend.
  #[must_use]
  pub const fn backend(&self) -> &B {
    &self.backend
  }

  /// Acquires a lock and executes the specified function (for reading).
  pub async fn read<R>(&self, f: impl FnOnce(&B::Guard<'_>) -> R) -> R {
    match self.backend.lock().await {
      | Ok(guard) => f(&guard),
      | Err(_) => panic!("Synchronized::read requires blocking to be allowed"),
    }
  }

  /// Acquires a lock and executes the specified function (for writing).
  pub async fn write<R>(&self, f: impl FnOnce(&mut B::Guard<'_>) -> R) -> R {
    match self.backend.lock().await {
      | Ok(mut guard) => f(&mut guard),
      | Err(_) => panic!("Synchronized::write requires blocking to be allowed"),
    }
  }

  /// Acquires a lock and returns a guard handle.
  pub async fn lock(&self) -> GuardHandle<B::Guard<'_>> {
    match self.backend.lock().await {
      | Ok(guard) => GuardHandle::new(guard),
      | Err(_) => panic!("Synchronized::lock requires blocking to be allowed"),
    }
  }
}

impl<B, T> Default for Synchronized<B, T>
where
  T: Default,
  B: SynchronizedMutexBackend<T>,
{
  fn default() -> Self {
    Self::new(T::default())
  }
}

impl<B, T> From<T> for Synchronized<B, T>
where
  T: Sized,
  B: SynchronizedMutexBackend<T>,
{
  fn from(value: T) -> Self {
    Self::new(value)
  }
}
