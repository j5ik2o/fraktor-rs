//! Re-entry detecting spin-based rwlock for debug/test instrumentation.
//!
//! Requires the `debug-locks` feature (which implies `std`) so that
//! `std::thread::current().id()` can distinguish same-thread re-entry
//! from legitimate cross-thread contention.
#![allow(cfg_std_forbid)]

#[cfg(test)]
mod tests;

use core::mem::ManuallyDrop;

use std::{
  collections::HashMap,
  sync::Mutex,
  thread,
  thread::ThreadId,
};

use super::{
  RwLockDriver, checked_rw_lock_read_guard::CheckedRwLockReadGuard,
  checked_rw_lock_write_guard::CheckedRwLockWriteGuard, spin_sync_rwlock::SpinSyncRwLock,
};

/// Tracks per-thread read counts and an exclusive write owner.
pub(super) struct OwnerState {
  /// Number of active read guards per thread.
  pub(super) reader_counts: HashMap<ThreadId, usize>,
  /// The thread holding the write lock, if any.
  pub(super) write_owner: Option<ThreadId>,
}

impl OwnerState {
  fn new() -> Self {
    Self { reader_counts: HashMap::new(), write_owner: None }
  }
}

/// Spin-based rwlock with thread-aware re-entry detection.
///
/// Wraps [`SpinSyncRwLock`] and tracks per-thread lock ownership.
/// Panics on:
/// - write re-entry (same thread calls `write()` while holding write)
/// - read→write upgrade (same thread calls `write()` while holding read)
/// - write→read downgrade (same thread calls `read()` while holding write)
///
/// `read()` → `read()` from the same thread is NOT flagged because
/// `spin::RwLock` supports concurrent/recursive readers.
pub struct CheckedSpinSyncRwLock<T> {
  pub(super) inner: SpinSyncRwLock<T>,
  /// Tracks per-thread read counts and exclusive write ownership.
  pub(super) owner: Mutex<OwnerState>,
}

unsafe impl<T: Send> Send for CheckedSpinSyncRwLock<T> {}
unsafe impl<T: Send + Sync> Sync for CheckedSpinSyncRwLock<T> {}

impl<T> CheckedSpinSyncRwLock<T> {
  /// Creates a new checked rwlock.
  #[must_use]
  pub fn new(value: T) -> Self {
    Self { inner: SpinSyncRwLock::new(value), owner: Mutex::new(OwnerState::new()) }
  }

  /// Acquires a shared read guard.
  ///
  /// # Panics
  ///
  /// Panics if the calling thread already holds a write lock.
  pub fn read(&self) -> CheckedRwLockReadGuard<'_, T> {
    let current = thread::current().id();
    {
      let state = self.owner.lock().unwrap_or_else(|e| e.into_inner());
      if state.write_owner == Some(current) {
        panic!("CheckedSpinSyncRwLock: read lock while write lock held");
      }
    }
    let guard = self.inner.read();
    {
      let mut state = self.owner.lock().unwrap_or_else(|e| e.into_inner());
      *state.reader_counts.entry(current).or_insert(0) += 1;
    }
    CheckedRwLockReadGuard { parent: self, guard: ManuallyDrop::new(guard) }
  }

  /// Acquires an exclusive write guard.
  ///
  /// # Panics
  ///
  /// Panics if the calling thread already holds any lock (read or write).
  pub fn write(&self) -> CheckedRwLockWriteGuard<'_, T> {
    let current = thread::current().id();
    {
      let state = self.owner.lock().unwrap_or_else(|e| e.into_inner());
      if state.write_owner == Some(current) {
        panic!("CheckedSpinSyncRwLock: re-entrant write lock detected");
      }
      if state.reader_counts.get(&current).copied().unwrap_or(0) > 0 {
        panic!("CheckedSpinSyncRwLock: write lock while read lock held");
      }
    }
    let guard = self.inner.write();
    {
      let mut state = self.owner.lock().unwrap_or_else(|e| e.into_inner());
      state.write_owner = Some(current);
    }
    CheckedRwLockWriteGuard { parent: self, guard: ManuallyDrop::new(guard) }
  }

  /// Consumes the rwlock and returns the inner value.
  pub fn into_inner(self) -> T {
    self.inner.into_inner()
  }
}

impl<T> RwLockDriver<T> for CheckedSpinSyncRwLock<T> {
  type ReadGuard<'a>
    = CheckedRwLockReadGuard<'a, T>
  where
    Self: 'a,
    T: 'a;
  type WriteGuard<'a>
    = CheckedRwLockWriteGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn new(value: T) -> Self {
    Self::new(value)
  }

  fn read(&self) -> Self::ReadGuard<'_> {
    CheckedSpinSyncRwLock::read(self)
  }

  fn write(&self) -> Self::WriteGuard<'_> {
    CheckedSpinSyncRwLock::write(self)
  }

  fn into_inner(self) -> T {
    CheckedSpinSyncRwLock::into_inner(self)
  }
}
