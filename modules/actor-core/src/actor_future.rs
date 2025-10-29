use core::cell::UnsafeCell;

/// Minimal future primitive used by the ask pattern.
pub struct ActorFuture<T> {
  value: UnsafeCell<Option<T>>,
}

unsafe impl<T: Send> Send for ActorFuture<T> {}
unsafe impl<T: Send> Sync for ActorFuture<T> {}

impl<T> ActorFuture<T> {
  /// Creates a new future in the pending state.
  #[must_use]
  pub const fn new() -> Self {
    Self { value: UnsafeCell::new(None) }
  }

  /// Completes the future with a value.
  pub fn complete(&self, value: T) {
    // ここでは単純な UnsafeCell を利用し、後続の実装で同期制御を追加する。
    unsafe {
      *self.value.get() = Some(value);
    }
  }

  /// Attempts to take the result if available.
  pub fn try_take(&self) -> Option<T> {
    unsafe { (*self.value.get()).take() }
  }
}
