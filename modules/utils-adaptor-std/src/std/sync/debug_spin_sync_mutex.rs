use core::{
  hash::{Hash, Hasher},
  ops::{Deref, DerefMut},
  sync::atomic::{AtomicU64, Ordering},
};
use std::{collections::hash_map::DefaultHasher, thread};

use fraktor_utils_core_rs::core::sync::{LockDriver, LockDriverFactory};

fn current_thread_id_u64() -> u64 {
  let mut hasher = DefaultHasher::new();
  thread::current().id().hash(&mut hasher);
  hasher.finish()
}

/// Re-entry detecting spin-based mutex for debug/test instrumentation.
pub struct DebugSpinSyncMutex<T> {
  inner: spin::Mutex<T>,
  owner: AtomicU64,
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
    DebugSpinSyncMutexGuard { parent: self, guard }
  }

  /// Consumes the mutex and returns the inner value.
  pub fn into_inner(self) -> T {
    self.inner.into_inner()
  }
}

/// Guard for [`DebugSpinSyncMutex`].
pub struct DebugSpinSyncMutexGuard<'a, T> {
  parent: &'a DebugSpinSyncMutex<T>,
  guard:  spin::MutexGuard<'a, T>,
}

impl<T> Deref for DebugSpinSyncMutexGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<T> DerefMut for DebugSpinSyncMutexGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.guard
  }
}

impl<T> Drop for DebugSpinSyncMutexGuard<'_, T> {
  fn drop(&mut self) {
    self.parent.owner.store(0, Ordering::Release);
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
    self.lock()
  }

  fn into_inner(self) -> T {
    self.into_inner()
  }
}

/// Factory for [`DebugSpinSyncMutex`].
pub struct DebugSpinSyncFactory;

impl LockDriverFactory for DebugSpinSyncFactory {
  type Driver<T> = DebugSpinSyncMutex<T>;
}

#[cfg(test)]
mod tests {
  use super::DebugSpinSyncMutex;

  #[test]
  fn locks_and_mutates_value() {
    let mutex = DebugSpinSyncMutex::new(1_u32);
    *mutex.lock() = 2;
    assert_eq!(*mutex.lock(), 2);
  }

  #[test]
  #[should_panic(expected = "DebugSpinSyncMutex detected same-thread re-entry")]
  fn panics_on_same_thread_reentry() {
    let mutex = DebugSpinSyncMutex::new(1_u32);
    let _guard = mutex.lock();
    let _reenter = mutex.lock();
  }
}
