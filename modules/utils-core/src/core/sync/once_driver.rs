/// Runtime-selectable write-once cell driver contract.
///
/// Mirrors the shape of [`crate::core::sync::LockDriver`] / [`crate::core::sync::RwLockDriver`] so
/// callers can swap the concrete backend (e.g. `SpinOnce`, a future `StdOnce`) through the
/// shared abstraction without coupling to a primitive crate.
pub trait OnceDriver<T>: Sized {
  /// Creates a fresh, uninitialized driver instance.
  fn new() -> Self;

  /// Initializes the cell exactly once and returns a reference to the stored value.
  fn call_once<F: FnOnce() -> T>(&self, f: F) -> &T;

  /// Returns the stored value if it has been initialized, otherwise `None`.
  fn get(&self) -> Option<&T>;

  /// Returns whether the cell has been initialized.
  fn is_completed(&self) -> bool;
}
