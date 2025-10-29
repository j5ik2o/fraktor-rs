//! Access helpers for shared backends guarded by mutex-like primitives.

use super::SharedError;
use crate::sync::{ArcShared, Shared, sync_mutex_like::SyncMutexLike};

/// Abstraction offering mutable access to shared backends.
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
