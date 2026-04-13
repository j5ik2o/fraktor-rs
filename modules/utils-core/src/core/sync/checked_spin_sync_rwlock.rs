//! Re-entry detecting spin-based rwlock for debug/test instrumentation.
//!
//! Requires the `debug-locks` feature (which implies `std`) so that
//! `std::thread::current().id()` can distinguish same-thread re-entry
//! from legitimate cross-thread contention.

#[cfg(test)]
mod tests;

use core::mem::ManuallyDrop;

use std::sync::Mutex;
use std::thread;

use super::{
  RwLockDriver, checked_rw_lock_read_guard::CheckedRwLockReadGuard,
  checked_rw_lock_write_guard::CheckedRwLockWriteGuard, spin_sync_rwlock::SpinSyncRwLock,
};

/// Lock state for a single thread.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ThreadLockState {
  /// Holding a read lock.
  Read,
  /// Holding a write lock.
  Write,
}

/// Per-thread lock ownership record.
pub(super) struct OwnerRecord {
  thread_id: thread::ThreadId,
  state:     ThreadLockState,
}

/// Spin-based rwlock with thread-aware re-entry detection.
///
/// Wraps [`SpinSyncRwLock`] and records the owning thread's lock state.
/// Panics on:
/// - write re-entry (same thread calls `write()` while holding write)
/// - read→write upgrade (same thread calls `write()` while holding read)
/// - write→read downgrade (same thread calls `read()` while holding write)
///
/// `read()` → `read()` from the same thread is NOT flagged because
/// `spin::RwLock` supports concurrent/recursive readers.
pub struct CheckedSpinSyncRwLock<T> {
  pub(super) inner: SpinSyncRwLock<T>,
  /// Tracks which thread (if any) holds the lock and in what mode.
  pub(super) owner: Mutex<Option<OwnerRecord>>,
}

unsafe impl<T: Send> Send for CheckedSpinSyncRwLock<T> {}
unsafe impl<T: Send + Sync> Sync for CheckedSpinSyncRwLock<T> {}

impl<T> CheckedSpinSyncRwLock<T> {
  /// Creates a new checked rwlock.
  #[must_use]
  pub fn new(value: T) -> Self {
    Self { inner: SpinSyncRwLock::new(value), owner: Mutex::new(None) }
  }

  /// Acquires a shared read guard.
  ///
  /// # Panics
  ///
  /// Panics if the calling thread already holds a write lock.
  pub fn read(&self) -> CheckedRwLockReadGuard<'_, T> {
    let current = thread::current().id();
    {
      let owner = self.owner.lock().unwrap_or_else(|e| e.into_inner());
      if let Some(record) = owner.as_ref() {
        if record.thread_id == current && record.state == ThreadLockState::Write {
          panic!("CheckedSpinSyncRwLock: read lock while write lock held");
        }
      }
    }
    let guard = self.inner.read();
    *self.owner.lock().unwrap_or_else(|e| e.into_inner()) =
      Some(OwnerRecord { thread_id: current, state: ThreadLockState::Read });
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
      let owner = self.owner.lock().unwrap_or_else(|e| e.into_inner());
      if let Some(record) = owner.as_ref() {
        if record.thread_id == current {
          let msg = match record.state {
            | ThreadLockState::Read => "write lock while read lock held",
            | ThreadLockState::Write => "re-entrant write lock detected",
          };
          panic!("CheckedSpinSyncRwLock: {msg}");
        }
      }
    }
    let guard = self.inner.write();
    *self.owner.lock().unwrap_or_else(|e| e.into_inner()) =
      Some(OwnerRecord { thread_id: current, state: ThreadLockState::Write });
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
