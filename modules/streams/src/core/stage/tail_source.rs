use alloc::vec::Vec;

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{Source, StreamError, StreamNotUsed};

#[cfg(test)]
mod tests;

/// Lazy tail source wrapper returned by `Flow::prefix_and_tail`.
pub struct TailSource<Out> {
  inner: ArcShared<SpinSyncMutex<Option<Source<Out, StreamNotUsed>>>>,
}

impl<Out> TailSource<Out>
where
  Out: Send + Sync + 'static,
{
  pub(crate) fn new(source: Source<Out, StreamNotUsed>) -> Self {
    Self { inner: ArcShared::new(SpinSyncMutex::new(Some(source))) }
  }

  /// Converts this wrapper into the underlying source exactly once.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::Failed`] when the source was already taken.
  pub fn into_source(self) -> Result<Source<Out, StreamNotUsed>, StreamError> {
    let mut guard = self.inner.lock();
    guard.take().ok_or(StreamError::Failed)
  }

  /// Collects all remaining tail elements.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when source execution fails.
  pub fn collect_values(self) -> Result<Vec<Out>, StreamError> {
    self.into_source()?.collect_values()
  }
}
