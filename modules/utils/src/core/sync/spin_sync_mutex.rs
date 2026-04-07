#[cfg(test)]
mod tests;

/// Thin wrapper around [`spin::Mutex`].
pub struct SpinSyncMutex<T>(spin::Mutex<T>);

unsafe impl<T: Send> Send for SpinSyncMutex<T> {}
unsafe impl<T: Send> Sync for SpinSyncMutex<T> {}

impl<T> SpinSyncMutex<T> {
  /// Creates a new spinlock-protected value.
  #[must_use]
  pub const fn new(value: T) -> Self {
    Self(spin::Mutex::new(value))
  }

  /// Returns a reference to the inner spin mutex.
  #[must_use]
  pub const fn as_inner(&self) -> &spin::Mutex<T> {
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
  /// When you suspect a specific call site is hitting re-entry deadlocks
  /// during testing, surgically replace `SpinSyncMutex<T>` with
  /// `fraktor_actor_adaptor_rs::std::debug::DebugSpinSyncMutex<T>`. The
  /// debug variant tracks the current owner thread and panics on
  /// re-entry from the same thread (while still permitting normal
  /// contention from other threads).
  pub fn lock(&self) -> spin::MutexGuard<'_, T> {
    self.0.lock()
  }
}
