use alloc::vec::Vec;

use super::{Flow, StreamDslError};

#[cfg(test)]
mod tests;

/// Substream DSL surface returned by flow substream operators.
pub struct FlowSubFlow<In, Out, Mat> {
  flow: Flow<In, Vec<Out>, Mat>,
}

impl<In, Out, Mat> FlowSubFlow<In, Out, Mat>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  pub(crate) const fn from_flow(flow: Flow<In, Vec<Out>, Mat>) -> Self {
    Self { flow }
  }

  /// Merges active substreams with unbounded parallelism semantics.
  #[must_use]
  pub fn merge_substreams(self) -> Flow<In, Out, Mat> {
    self.flow.merge_substreams()
  }

  /// Merges active substreams with explicit parallelism.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn merge_substreams_with_parallelism(self, parallelism: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    self.flow.merge_substreams_with_parallelism(parallelism)
  }

  /// Concatenates active substreams.
  #[must_use]
  pub fn concat_substreams(self) -> Flow<In, Out, Mat> {
    self.flow.concat_substreams()
  }
}
