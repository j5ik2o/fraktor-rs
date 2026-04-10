#[cfg(test)]
mod tests;

use core::{
  hash::{Hash, Hasher},
  mem::ManuallyDrop,
  sync::atomic::{AtomicU64, Ordering},
};
use std::{collections::hash_map::DefaultHasher, thread};

use fraktor_utils_core_rs::core::sync::LockDriver;

use super::debug_spin_sync_mutex_guard::DebugSpinSyncMutexGuard;

fn current_thread_id_u64() -> u64 {
  let mut hasher = DefaultHasher::new();
  thread::current().id().hash(&mut hasher);
  hasher.finish()
}

/// Re-entry detecting spin-based mutex for debug/test instrumentation.
pub struct DebugSpinSyncMutex<T> {
  pub(super) inner: spin::Mutex<T>,
  pub(super) owner: AtomicU64,
}

unsafe impl<T: Send> Send for DebugSpinSyncMutex<T> {}
unsafe impl<T: Send> Sync for DebugSpinSyncMutex<T> {}

impl<T> DebugSpinSyncMutex<T> {
  /// Creates a new debug mutex.
  #[must_use]
  pub const fn new(value: T) -> Self {
    Self { inner: spin::Mutex::new(value), owner: AtomicU64::new(0) }
  }

  /// Acquires the mutex, panicking on same-thread re-entry.
  pub fn lock(&self) -> DebugSpinSyncMutexGuard<'_, T> {
    let current = current_thread_id_u64();
    let observed = self.owner.load(Ordering::Acquire);
    assert_ne!(observed, current, "DebugSpinSyncMutex detected same-thread re-entry");
    let guard = self.inner.lock();
    self.owner.store(current, Ordering::Release);
    DebugSpinSyncMutexGuard { parent: self, guard: ManuallyDrop::new(guard) }
  }

  /// Consumes the mutex and returns the inner value.
  pub fn into_inner(self) -> T {
    self.inner.into_inner()
  }
}

impl<T> LockDriver<T> for DebugSpinSyncMutex<T> {
  type Guard<'a>
    = DebugSpinSyncMutexGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn new(value: T) -> Self {
    Self::new(value)
  }

  fn lock(&self) -> Self::Guard<'_> {
    DebugSpinSyncMutex::lock(self)
  }

  fn into_inner(self) -> T {
    DebugSpinSyncMutex::into_inner(self)
  }
}
