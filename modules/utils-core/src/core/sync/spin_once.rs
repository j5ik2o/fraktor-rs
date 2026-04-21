#[cfg(test)]
mod tests;

use spin::Once;

use crate::core::sync::OnceDriver;

/// Thin wrapper around [`spin::Once`], acting as the `spin` backend for the `OnceDriver`
/// abstraction. This is the sole place inside `utils-core` that uses `spin::Once` directly;
/// upstream callers must route through [`crate::core::sync::SyncOnce`].
pub struct SpinOnce<T>(Once<T>);

unsafe impl<T: Send> Send for SpinOnce<T> {}
unsafe impl<T: Send + Sync> Sync for SpinOnce<T> {}

impl<T> SpinOnce<T> {
  /// Creates a new, uninitialized `SpinOnce`.
  #[must_use]
  pub const fn new() -> Self {
    Self(Once::new())
  }

  /// Initializes the cell exactly once and returns a reference to the stored value.
  pub fn call_once<F: FnOnce() -> T>(&self, f: F) -> &T {
    self.0.call_once(f)
  }

  /// Returns the stored value if it has been initialized.
  #[must_use]
  pub fn get(&self) -> Option<&T> {
    self.0.get()
  }

  /// Returns whether the cell has been initialized.
  #[must_use]
  pub fn is_completed(&self) -> bool {
    self.0.is_completed()
  }
}

impl<T> Default for SpinOnce<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T> OnceDriver<T> for SpinOnce<T> {
  fn new() -> Self {
    Self::new()
  }

  fn call_once<F: FnOnce() -> T>(&self, f: F) -> &T {
    self.call_once(f)
  }

  fn get(&self) -> Option<&T> {
    self.get()
  }

  fn is_completed(&self) -> bool {
    self.is_completed()
  }
}
