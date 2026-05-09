use super::{SharedLock, WeakShared, shared_lock::SharedLockBackend};

/// Weak counterpart of [`SharedLock`].
pub struct WeakSharedLock<T> {
  inner: WeakShared<dyn SharedLockBackend<T>>,
}

impl<T> WeakSharedLock<T>
where
  T: Send + 'static,
{
  #[must_use]
  pub(crate) const fn new(inner: WeakShared<dyn SharedLockBackend<T>>) -> Self {
    Self { inner }
  }

  /// Attempts to upgrade the weak reference to a [`SharedLock`].
  #[must_use]
  pub fn upgrade(&self) -> Option<SharedLock<T>> {
    self.inner.upgrade().map(SharedLock::from_inner)
  }
}

impl<T> Clone for WeakSharedLock<T> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
