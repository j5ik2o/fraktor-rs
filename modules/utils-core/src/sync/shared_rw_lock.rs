#[cfg(test)]
mod tests;

use core::marker::PhantomData;

use super::{ArcShared, RwLockDriver, SharedAccess, WeakSharedRwLock};

pub(crate) trait SharedRwLockBackend<T>: Send + Sync {
  fn with_read(&self, f: &mut dyn FnMut(&T));
  fn with_write(&self, f: &mut dyn FnMut(&mut T));
}

struct RwLockDriverBackend<T, D>
where
  D: RwLockDriver<T>, {
  inner: D,
  _pd:   PhantomData<fn() -> T>,
}

impl<T, D> RwLockDriverBackend<T, D>
where
  D: RwLockDriver<T>,
{
  fn new(value: T) -> Self {
    Self { inner: D::new(value), _pd: PhantomData }
  }
}

impl<T, D> SharedRwLockBackend<T> for RwLockDriverBackend<T, D>
where
  T: Send + Sync + 'static,
  D: RwLockDriver<T> + Send + Sync + 'static,
{
  fn with_read(&self, f: &mut dyn FnMut(&T)) {
    let guard = self.inner.read();
    f(&guard);
  }

  fn with_write(&self, f: &mut dyn FnMut(&mut T)) {
    let mut guard = self.inner.write();
    f(&mut guard);
  }
}

/// Shared rwlock-backed handle that keeps the driver choice in `utils-core`.
pub struct SharedRwLock<T> {
  inner: ArcShared<dyn SharedRwLockBackend<T>>,
}

impl<T> SharedRwLock<T>
where
  T: Send + Sync + 'static,
{
  #[must_use]
  pub(crate) const fn from_inner(inner: ArcShared<dyn SharedRwLockBackend<T>>) -> Self {
    Self { inner }
  }

  /// Creates a new shared rwlock from the supplied value using the requested driver.
  #[must_use]
  pub fn new_with_driver<D>(value: T) -> Self
  where
    D: RwLockDriver<T> + Send + Sync + 'static, {
    let backend = ArcShared::new(RwLockDriverBackend::<T, D>::new(value));
    #[cfg(feature = "unsize")]
    let inner: ArcShared<dyn SharedRwLockBackend<T>> = backend;
    #[cfg(not(feature = "unsize"))]
    let inner = backend.into_dyn(|value| value as &dyn SharedRwLockBackend<T>);
    Self { inner }
  }

  /// Executes `f` while holding a shared read lock.
  ///
  /// # Panics
  ///
  /// Panics if the internal backend violates the contract and fails to invoke
  /// the closure exactly once.
  pub fn with_read<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    let mut f = Some(f);
    let mut result = None;
    self.inner.with_read(&mut |value| {
      let Some(callback) = f.take() else {
        panic!("shared rwlock read closure should be called once");
      };
      result = Some(callback(value));
    });
    match result {
      | Some(result) => result,
      | None => panic!("shared rwlock backend must invoke the read closure exactly once"),
    }
  }

  /// Executes `f` while holding an exclusive write lock.
  ///
  /// # Panics
  ///
  /// Panics if the internal backend violates the contract and fails to invoke
  /// the closure exactly once.
  pub fn with_write<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
    let mut f = Some(f);
    let mut result = None;
    self.inner.with_write(&mut |value| {
      let Some(callback) = f.take() else {
        panic!("shared rwlock write closure should be called once");
      };
      result = Some(callback(value));
    });
    match result {
      | Some(result) => result,
      | None => panic!("shared rwlock backend must invoke the write closure exactly once"),
    }
  }

  /// Creates a weak reference to this shared rwlock.
  #[must_use]
  pub fn downgrade(&self) -> WeakSharedRwLock<T> {
    WeakSharedRwLock::new(self.inner.downgrade())
  }

  /// Returns `true` if both handles point to the same allocation.
  #[must_use]
  pub fn ptr_eq(this: &Self, other: &Self) -> bool {
    ArcShared::ptr_eq(&this.inner, &other.inner)
  }
}

impl<T> Clone for SharedRwLock<T> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<T> SharedAccess<T> for SharedRwLock<T>
where
  T: Send + Sync + 'static,
{
  fn with_read<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    Self::with_read(self, f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
    Self::with_write(self, f)
  }
}
