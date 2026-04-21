#[cfg(test)]
mod tests;

use crate::core::sync::SpinOnce;

/// Public write-once cell abstraction that upstream crates (e.g. `actor-*`) depend on.
///
/// Mirrors the `LockDriver` → `SpinSyncMutex` → `SharedLock` layering: `SyncOnce` is the public
/// abstraction, while the concrete backend lives in `SpinOnce` so that the primitive `spin` crate
/// stays confined to `utils-core`.
///
/// Unlike `SharedLock`, `SyncOnce` does not wrap an `ArcShared` layer. Once-cells are written once
/// and then read without synchronization, so the extra shared-ownership layer provides no benefit
/// for the typical usage pattern. Callers that need shared ownership can wrap `SyncOnce` inside
/// their own `ArcShared<SyncOnce<T>>`.
pub struct SyncOnce<T> {
  inner: SpinOnce<T>,
}

impl<T> SyncOnce<T> {
  /// Creates a new, uninitialized `SyncOnce`.
  #[must_use]
  pub const fn new() -> Self {
    Self { inner: SpinOnce::new() }
  }

  /// Initializes the cell exactly once and returns a reference to the stored value.
  pub fn call_once<F: FnOnce() -> T>(&self, f: F) -> &T {
    self.inner.call_once(f)
  }

  /// Returns the stored value if it has been initialized.
  #[must_use]
  pub fn get(&self) -> Option<&T> {
    self.inner.get()
  }

  /// Returns whether the cell has been initialized.
  #[must_use]
  pub fn is_completed(&self) -> bool {
    self.inner.is_completed()
  }
}

impl<T> Default for SyncOnce<T> {
  fn default() -> Self {
    Self::new()
  }
}
