use alloc::vec::Vec;

use super::{Source, StreamDslError};

#[cfg(test)]
mod tests;

/// Substream DSL surface returned by source substream operators.
pub struct SourceSubFlow<Out, Mat> {
  source: Source<Vec<Out>, Mat>,
}

impl<Out, Mat> SourceSubFlow<Out, Mat>
where
  Out: Send + Sync + 'static,
{
  pub(crate) const fn from_source(source: Source<Vec<Out>, Mat>) -> Self {
    Self { source }
  }

  /// Merges active substreams with unbounded parallelism semantics.
  #[must_use]
  pub fn merge_substreams(self) -> Source<Out, Mat> {
    self.source.merge_substreams()
  }

  /// Merges active substreams with explicit parallelism.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn merge_substreams_with_parallelism(self, parallelism: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    self.source.merge_substreams_with_parallelism(parallelism)
  }

  /// Concatenates active substreams.
  #[must_use]
  pub fn concat_substreams(self) -> Source<Out, Mat> {
    self.source.concat_substreams()
  }
}
