use core::ops::Deref;

/// Shared ownership abstraction used across runtimes.
pub trait Shared<T: ?Sized>: Clone + Deref<Target = T> {
  /// Attempt to unwrap the shared value.
  fn try_unwrap(self) -> Result<T, Self>
  where
    T: Sized, {
    Err(self)
  }

  /// Execute the provided closure with a shared reference to the inner value.
  fn with_ref<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    f(self.deref())
  }
}
