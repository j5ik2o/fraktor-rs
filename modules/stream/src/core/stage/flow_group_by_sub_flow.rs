use super::{StreamDslError, flow::Flow, sink::Sink};

#[cfg(test)]
mod tests;

/// Substream surface returned by `group_by`.
pub struct FlowGroupBySubFlow<In, Key, Out, Mat> {
  flow: super::flow::Flow<In, (Key, Out), Mat>,
}

impl<In, Key, Out, Mat> FlowGroupBySubFlow<In, Key, Out, Mat>
where
  In: Send + Sync + 'static,
  Key: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  pub(crate) const fn from_flow(flow: Flow<In, (Key, Out), Mat>) -> Self {
    Self { flow }
  }

  /// Merges grouped substreams back into the parent flow.
  #[must_use]
  pub fn merge_substreams(self) -> Flow<In, Out, Mat> {
    self.flow.map(|(_, value)| value)
  }

  /// Merges grouped substreams with explicit parallelism.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn merge_substreams_with_parallelism(self, parallelism: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    if parallelism == 0 {
      return Err(StreamDslError::InvalidArgument {
        name:   "parallelism",
        value:  0,
        reason: "must be greater than zero",
      });
    }
    Ok(self.flow.map(|(_, value)| value))
  }

  /// Concatenates grouped substreams sequentially.
  #[must_use]
  pub fn concat_substreams(self) -> Flow<In, Out, Mat> {
    self.flow.map(|(_, value)| value)
  }

  /// Connects this sub-flow to a sink, merging substreams first.
  #[must_use]
  pub fn to<Mat2>(self, sink: Sink<Out, Mat2>) -> Sink<In, Mat> {
    self.merge_substreams().to(sink)
  }

  /// Maps each element's value within grouped substreams, preserving keys.
  #[must_use]
  pub fn map<T, F>(self, mut func: F) -> FlowGroupBySubFlow<In, Key, T, Mat>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> T + Send + Sync + 'static, {
    FlowGroupBySubFlow::from_flow(self.flow.map(move |(key, value)| (key, func(value))))
  }

  /// Filters elements within grouped substreams by value, preserving keys.
  #[must_use]
  pub fn filter<F>(self, mut predicate: F) -> FlowGroupBySubFlow<In, Key, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    FlowGroupBySubFlow::from_flow(self.flow.filter(move |(_, value)| predicate(value)))
  }
}
