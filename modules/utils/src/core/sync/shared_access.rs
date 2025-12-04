//! Access helpers for shared backends guarded by mutex-like primitives.

use crate::core::sync::{ArcShared, shared::Shared, sync_mutex_like::SyncMutexLike};

#[cfg(test)]
mod tests;

/// Abstraction offering mutable access to shared backends.
///
/// # Design
///
/// - Callers do not need to bind the shared handle (e.g., `ArcShared<Mutex<T>>`) as `mut`.
/// - Internally calls `SyncMutexLike::lock(&self)` and performs mutations via the acquired guard.
/// - If you want to switch to a design where the lock is held externally, do not change this trait
///   to take `&mut self`. Instead, introduce a dedicated handler type to encapsulate that
///   responsibility.
pub trait SharedAccess<B> {
  /// Executes the provided closure with read-only access to the backend.
  fn with_read<R>(&self, f: impl FnOnce(&B) -> R) -> R;

  /// Executes the provided closure with mutable access to the backend.
  fn with_write<R>(&self, f: impl FnOnce(&mut B) -> R) -> R;
}

impl<B, M> SharedAccess<B> for ArcShared<M>
where
  M: SyncMutexLike<B>,
{
  fn with_read<R>(&self, f: impl FnOnce(&B) -> R) -> R {
    Shared::with_ref(self, |mutex| {
      let guard = mutex.lock();
      f(&guard)
    })
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut B) -> R) -> R {
    Shared::with_ref(self, |mutex| {
      let mut guard = mutex.lock();
      f(&mut guard)
    })
  }
}
