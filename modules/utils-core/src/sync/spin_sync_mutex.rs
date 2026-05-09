#[cfg(test)]
mod tests;

use spin::{Mutex, MutexGuard};

use crate::sync::LockDriver;

/// Thin wrapper around [`Mutex`].
pub struct SpinSyncMutex<T>(Mutex<T>);

unsafe impl<T: Send> Send for SpinSyncMutex<T> {}
unsafe impl<T: Send> Sync for SpinSyncMutex<T> {}

impl<T> SpinSyncMutex<T> {
  /// Creates a new spinlock-protected value.
  #[must_use]
  pub const fn new(value: T) -> Self {
    Self(Mutex::new(value))
  }

  /// Returns a reference to the inner spin mutex.
  #[must_use]
  pub const fn as_inner(&self) -> &Mutex<T> {
    &self.0
  }

  /// Consumes the wrapper and returns the underlying value.
  pub fn into_inner(self) -> T {
    self.0.into_inner()
  }

  /// Locks the mutex and returns a guard to the protected value.
  ///
  /// # Deadlock
  ///
  /// `spin::Mutex` is **NOT reentrant**. Recursively locking on the same
  /// thread (e.g., calling another `lock()` while still holding the guard
  /// returned by an earlier call) will spin forever and deadlock the
  /// thread. The CPU spins at 100% and no progress is made.
  ///
  /// To avoid re-entry bugs, prefer the `AShared` `with_read` /
  /// `with_write` closure-based API documented in
  /// `.agents/rules/rust/immutability-policy.md`. The closure form makes
  /// it structurally harder to nest lock acquisitions because the callee
  /// cannot easily call back into the same shared state.
  ///
  /// A drop-in instrumented variant for re-entry detection during testing can
  /// be reintroduced on top of the current `LockDriver` abstraction when the
  /// project needs it again; until then, deadlock symptoms must be diagnosed
  /// via stack traces of the spinning thread.
  pub fn lock(&self) -> MutexGuard<'_, T> {
    self.0.lock()
  }
}

impl<T> LockDriver<T> for SpinSyncMutex<T> {
  type Guard<'a>
    = MutexGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn new(value: T) -> Self {
    Self::new(value)
  }

  fn lock(&self) -> Self::Guard<'_> {
    self.lock()
  }

  fn into_inner(self) -> T {
    self.into_inner()
  }
}
