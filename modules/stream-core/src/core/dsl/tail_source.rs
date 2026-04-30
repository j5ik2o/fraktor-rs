use fraktor_utils_core_rs::core::sync::SpinSyncMutex;

use super::{StreamNotUsed, source::Source};

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
}
