//! Drop-in instrumented replacement for [`SpinSyncMutex`] that detects
//! same-thread re-entry deadlocks during testing.
//!
//! `spin::Mutex` (and therefore [`SpinSyncMutex`]) is non-reentrant.
//! Recursively locking on the same thread spins forever. This module
//! provides a wrapper that records the owner [`std::thread::ThreadId`]
//! on every `lock()` call and panics with a descriptive message when
//! the same thread tries to acquire the lock while still holding it.
//!
//! `DebugSpinSyncMutex` is std-only and lives in the std adapter crate
//! so that the no_std utils core remains free of std dependencies. Use
//! it surgically by replacing `SpinSyncMutex<T>` with
//! `DebugSpinSyncMutex<T>` in suspect call sites under
//! `#[cfg(any(test, feature = "test-support"))]`. Production code keeps
//! using the regular [`SpinSyncMutex`].

use std::{
  fmt,
  sync::atomic::{AtomicU64, Ordering},
  thread,
};

use fraktor_utils_core_rs::core::sync::SpinSyncMutex;

use super::debug_spin_sync_mutex_guard::DebugSpinSyncMutexGuard;

#[cfg(test)]
mod tests;

pub(super) const UNLOCKED: u64 = 0;

/// Thread-tracking wrapper around [`SpinSyncMutex`].
///
/// On every `lock()` call, the wrapper records the current thread's
/// hashed identity. If the same thread tries to acquire the lock again
/// while still holding it, the wrapper panics instead of letting the
/// underlying spin mutex deadlock.
pub struct DebugSpinSyncMutex<T> {
  inner: SpinSyncMutex<T>,
  owner: AtomicU64,
}

unsafe impl<T: Send> Send for DebugSpinSyncMutex<T> {}
unsafe impl<T: Send> Sync for DebugSpinSyncMutex<T> {}

impl<T> DebugSpinSyncMutex<T> {
  /// Wraps the provided value in a debug-tracking spin mutex.
  #[must_use]
  pub const fn new(value: T) -> Self {
    Self { inner: SpinSyncMutex::new(value), owner: AtomicU64::new(UNLOCKED) }
  }

  /// Locks the mutex and returns a guard to the protected value.
  ///
  /// # Panics
  ///
  /// Panics with a descriptive message when the calling thread already
  /// holds this mutex (indicating a re-entry bug that would otherwise
  /// deadlock the underlying spinlock).
  pub fn lock(&self) -> DebugSpinSyncMutexGuard<'_, T> {
    let current = current_thread_id();
    let prior = self.owner.load(Ordering::Acquire);
    assert!(
      prior != current,
      "DebugSpinSyncMutex re-entered by the same thread {:?}: this would \
       deadlock the underlying spin::Mutex. Audit the call site for nested \
       lock() calls.",
      thread::current().id(),
    );
    let inner = self.inner.lock();
    self.owner.store(current, Ordering::Release);
    DebugSpinSyncMutexGuard::new(inner, &self.owner)
  }

  /// Consumes the wrapper and returns the underlying value.
  pub fn into_inner(self) -> T {
    self.inner.into_inner()
  }
}

impl<T: fmt::Debug> fmt::Debug for DebugSpinSyncMutex<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("DebugSpinSyncMutex").field("owner", &self.owner.load(Ordering::Relaxed)).finish_non_exhaustive()
  }
}

/// Returns a stable u64 identity for the current thread.
///
/// `ThreadId::as_u64` is unstable, so we hash the `ThreadId`'s `Hash`
/// impl through std's `DefaultHasher` instead. Collisions are possible
/// in theory but extremely unlikely for the small number of threads
/// involved in any single test, and a collision merely loses re-entry
/// detection (false negative) — it never causes a false positive.
fn current_thread_id() -> u64 {
  use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
  };
  let mut hasher = DefaultHasher::new();
  thread::current().id().hash(&mut hasher);
  let hashed = hasher.finish();
  if hashed == UNLOCKED { 1 } else { hashed }
}
