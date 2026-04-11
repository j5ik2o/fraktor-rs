#[cfg(test)]
mod tests;

use core::marker::PhantomData;

use super::{ArcShared, DefaultLockDriver, LockDriver, SharedAccess, WeakSharedLock};

pub(crate) trait SharedLockBackend<T>: Send + Sync {
  fn with_lock(&self, f: &mut dyn FnMut(&mut T));
}

struct LockDriverBackend<T, D>
where
  D: LockDriver<T>, {
  inner: D,
  _pd:   PhantomData<fn() -> T>,
}

impl<T, D> LockDriverBackend<T, D>
where
  D: LockDriver<T>,
{
  fn new(value: T) -> Self {
    Self { inner: D::new(value), _pd: PhantomData }
  }
}

impl<T, D> SharedLockBackend<T> for LockDriverBackend<T, D>
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
  #[must_use]
  pub(crate) const fn from_inner(inner: ArcShared<dyn SharedLockBackend<T>>) -> Self {
    Self { inner }
  }

  /// Creates a new shared lock backed by the workspace's compile-time
  /// selected default driver.
  ///
  /// This is the canonical constructor: it picks
  /// [`DefaultLockDriver<T>`](super::DefaultLockDriver) (resolved through
  /// `default-lock-*` Cargo features) so that callers do not have to plumb a
  /// driver type or a runtime `LockProvider` through their constructors.
  ///
  /// Use [`Self::new_with_driver`] only when a specific driver (e.g. a debug
  /// instrumented mutex) is required at the construction site.
  #[must_use]
  pub fn new(value: T) -> Self {
    Self::new_with_driver::<DefaultLockDriver<T>>(value)
  }

  /// Creates a new shared lock from the supplied value using the requested driver.
  #[must_use]
  pub fn new_with_driver<D>(value: T) -> Self
  where
    D: LockDriver<T> + Send + Sync + 'static, {
    let backend = ArcShared::new(LockDriverBackend::<T, D>::new(value));
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

  /// Creates a weak reference to this shared lock.
  #[must_use]
  pub fn downgrade(&self) -> WeakSharedLock<T> {
    WeakSharedLock::new(self.inner.downgrade())
  }

  /// Returns `true` if both handles point to the same allocation.
  #[must_use]
  pub fn ptr_eq(this: &Self, other: &Self) -> bool {
    ArcShared::ptr_eq(&this.inner, &other.inner)
  }
}

impl<T> Clone for SharedLock<T> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<T> SharedAccess<T> for SharedLock<T>
where
  T: Send + 'static,
{
  fn with_read<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    Self::with_lock(self, |value| f(value))
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
    Self::with_lock(self, f)
  }
}
