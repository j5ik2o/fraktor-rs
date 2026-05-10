use alloc::vec::Vec;

use super::{StreamDslError, sink::Sink, source::Source};
use crate::materialization::RunnableGraph;

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

  /// Connects this sub-flow to a sink, merging substreams first.
  #[must_use]
  pub fn to<Mat2>(self, sink: Sink<Out, Mat2>) -> RunnableGraph<Mat> {
    self.merge_substreams().to(sink)
  }

  /// Maps each element inside every substream.
  #[must_use]
  pub fn map<T, F>(self, func: F) -> SourceSubFlow<T, Mat>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> T + Send + Sync + Clone + 'static, {
    SourceSubFlow::from_source(self.source.map(move |values| {
      let mut mapper = func.clone();
      values.into_iter().map(&mut mapper).collect()
    }))
  }

  /// Filters each substream independently.
  #[must_use]
  pub fn filter<F>(self, predicate: F) -> SourceSubFlow<Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + Clone + 'static, {
    SourceSubFlow::from_source(self.source.map(move |values| {
      let mut checker = predicate.clone();
      values.into_iter().filter(|value| checker(value)).collect()
    }))
  }

  /// Drops the first `count` elements from every substream.
  #[must_use]
  pub fn drop(self, count: usize) -> SourceSubFlow<Out, Mat> {
    SourceSubFlow::from_source(self.source.map(move |values| values.into_iter().skip(count).collect()))
  }

  /// Takes the first `count` elements from every substream.
  #[must_use]
  pub fn take(self, count: usize) -> SourceSubFlow<Out, Mat> {
    SourceSubFlow::from_source(self.source.map(move |values| values.into_iter().take(count).collect()))
  }

  /// Drops elements from each substream while `predicate` returns `true`.
  #[must_use]
  pub fn drop_while<F>(self, predicate: F) -> SourceSubFlow<Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + Clone + 'static, {
    SourceSubFlow::from_source(self.source.map(move |values| {
      let mut checker = predicate.clone();
      values.into_iter().skip_while(|value| checker(value)).collect()
    }))
  }

  /// Takes elements from each substream while `predicate` returns `true`.
  #[must_use]
  pub fn take_while<F>(self, predicate: F) -> SourceSubFlow<Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + Clone + 'static, {
    SourceSubFlow::from_source(self.source.map(move |values| {
      let mut checker = predicate.clone();
      values.into_iter().take_while(|value| checker(value)).collect()
    }))
  }
}
