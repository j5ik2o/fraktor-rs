use alloc::vec::Vec;

use fraktor_utils_rs::core::sync::sync_mutex_like::SpinSyncMutex;

use super::{Source, StreamError, StreamNotUsed};

#[cfg(test)]
mod tests;

/// Lazy tail source wrapper returned by `Flow::prefix_and_tail`.
pub struct TailSource<Out> {
  inner: SpinSyncMutex<Source<Out, StreamNotUsed>>,
}

impl<Out> TailSource<Out>
where
  Out: Send + Sync + 'static,
{
  pub(crate) const fn new(source: Source<Out, StreamNotUsed>) -> Self {
    Self { inner: SpinSyncMutex::new(source) }
  }

  /// Converts this wrapper into the underlying source exactly once.
  #[must_use]
  pub fn into_source(self) -> Source<Out, StreamNotUsed> {
    self.inner.into_inner()
  }

  /// Collects all remaining tail elements.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when source execution fails.
  pub fn collect_values(self) -> Result<Vec<Out>, StreamError> {
    self.into_source().collect_values()
  }
}
