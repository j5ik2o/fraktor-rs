//! Access helpers for shared backends guarded by mutex-like primitives.

use crate::core::sync::{ArcShared, SharedError, shared::Shared, sync_mutex_like::SyncMutexLike};

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
  /// Executes the provided closure with mutable access to the backend.
  ///
  /// # Errors
  ///
  /// Returns a `SharedError` when the shared backend cannot be accessed, such as when the state is
  /// poisoned or a borrow would conflict.
  fn with_mut<R>(&self, f: impl FnOnce(&mut B) -> R) -> Result<R, SharedError>;
}

impl<B, M> SharedAccess<B> for ArcShared<M>
where
  M: SyncMutexLike<B>,
{
  fn with_mut<R>(&self, f: impl FnOnce(&mut B) -> R) -> Result<R, SharedError> {
    let result = self.with_ref(|mutex| {
      let mut guard = mutex.lock();
      f(&mut guard)
    });
    Ok(result)
  }
}
