use super::{SharedRwLock, WeakShared, shared_rw_lock::SharedRwLockBackend};

/// Weak counterpart of [`SharedRwLock`].
pub struct WeakSharedRwLock<T> {
  inner: WeakShared<dyn SharedRwLockBackend<T>>,
}

impl<T> WeakSharedRwLock<T>
where
  T: Send + Sync + 'static,
{
  #[must_use]
  pub(crate) const fn new(inner: WeakShared<dyn SharedRwLockBackend<T>>) -> Self {
    Self { inner }
  }

  /// Attempts to upgrade the weak reference to a [`SharedRwLock`].
  #[must_use]
  pub fn upgrade(&self) -> Option<SharedRwLock<T>> {
    self.inner.upgrade().map(SharedRwLock::from_inner)
  }
}

impl<T> Clone for WeakSharedRwLock<T> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
