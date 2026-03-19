use alloc::vec::Vec;

use super::{StreamDslError, flow::Flow};

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

  /// Maps each element inside every substream.
  #[must_use]
  pub fn map<T, F>(self, func: F) -> FlowSubFlow<In, T, Mat>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> T + Send + Sync + Clone + 'static, {
    FlowSubFlow::from_flow(self.flow.map(move |values| {
      let mut mapper = func.clone();
      values.into_iter().map(&mut mapper).collect()
    }))
  }

  /// Filters each substream independently.
  #[must_use]
  pub fn filter<F>(self, predicate: F) -> FlowSubFlow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + Clone + 'static, {
    FlowSubFlow::from_flow(self.flow.map(move |values| {
      let mut checker = predicate.clone();
      values.into_iter().filter(|value| checker(value)).collect()
    }))
  }

  /// Drops the first `count` elements from every substream.
  #[must_use]
  pub fn drop(self, count: usize) -> FlowSubFlow<In, Out, Mat> {
    FlowSubFlow::from_flow(self.flow.map(move |values| values.into_iter().skip(count).collect()))
  }

  /// Takes the first `count` elements from every substream.
  #[must_use]
  pub fn take(self, count: usize) -> FlowSubFlow<In, Out, Mat> {
    FlowSubFlow::from_flow(self.flow.map(move |values| values.into_iter().take(count).collect()))
  }

  /// Drops elements from each substream while `predicate` returns `true`.
  #[must_use]
  pub fn drop_while<F>(self, predicate: F) -> FlowSubFlow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + Clone + 'static, {
    FlowSubFlow::from_flow(self.flow.map(move |values| {
      let mut checker = predicate.clone();
      values.into_iter().skip_while(|value| checker(value)).collect()
    }))
  }

  /// Takes elements from each substream while `predicate` returns `true`.
  #[must_use]
  pub fn take_while<F>(self, predicate: F) -> FlowSubFlow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + Clone + 'static, {
    FlowSubFlow::from_flow(self.flow.map(move |values| {
      let mut checker = predicate.clone();
      values.into_iter().take_while(|value| checker(value)).collect()
    }))
  }
}
