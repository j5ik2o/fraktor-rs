use super::{ArcShared, LockDriver, RuntimeMutex};

trait SharedLockBackend<T>: Send + Sync {
  fn with_lock(&self, f: &mut dyn FnMut(&mut T));
}

struct RuntimeMutexSharedLockBackend<T, D>
where
  D: LockDriver<T>, {
  inner: RuntimeMutex<T, D>,
}

impl<T, D> RuntimeMutexSharedLockBackend<T, D>
where
  D: LockDriver<T>,
{
  fn new(value: T) -> Self {
    Self { inner: RuntimeMutex::<T, D>::new_with_driver(value) }
  }
}

impl<T, D> SharedLockBackend<T> for RuntimeMutexSharedLockBackend<T, D>
where
  T: Send + 'static,
  D: LockDriver<T> + Send + Sync + 'static,
{
  fn with_lock(&self, f: &mut dyn FnMut(&mut T)) {
    let mut guard = self.inner.lock();
    f(&mut guard);
  }
}

/// Shared mutex-backed handle that keeps the driver choice in `utils-core`.
pub struct SharedLock<T> {
  inner: ArcShared<dyn SharedLockBackend<T>>,
}

impl<T> SharedLock<T>
where
  T: Send + 'static,
{
  /// Creates a new shared lock from the supplied value using the requested driver.
  #[must_use]
  pub fn new_with_driver<D>(value: T) -> Self
  where
    D: LockDriver<T> + Send + Sync + 'static, {
    let backend = ArcShared::new(RuntimeMutexSharedLockBackend::<T, D>::new(value));
    #[cfg(feature = "unsize")]
    let inner: ArcShared<dyn SharedLockBackend<T>> = backend;
    #[cfg(not(feature = "unsize"))]
    let inner = backend.into_dyn(|value| value as &dyn SharedLockBackend<T>);
    Self { inner }
  }

  /// Executes `f` while holding the underlying lock.
  ///
  /// # Panics
  ///
  /// Panics if the internal backend violates the contract and fails to invoke
  /// the closure exactly once.
  pub fn with_lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
    let mut f = Some(f);
    let mut result = None;
    self.inner.with_lock(&mut |value| {
      let Some(callback) = f.take() else {
        panic!("shared lock closure should be called once");
      };
      result = Some(callback(value));
    });
    match result {
      | Some(result) => result,
      | None => panic!("shared lock backend must invoke the closure exactly once"),
    }
  }

  /// Executes `f` with read-only access under the underlying lock.
  pub fn with_read<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    self.with_lock(|value| f(value))
  }
}

impl<T> Clone for SharedLock<T> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
