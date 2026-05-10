use super::{sink::Sink, source::Source};
use crate::materialization::RunnableGraph;

#[cfg(test)]
mod tests;

/// Substream surface returned by `group_by`.
pub struct SourceGroupBySubFlow<Key, Out, Mat> {
  source: Source<(Key, Out), Mat>,
}

impl<Key, Out, Mat> SourceGroupBySubFlow<Key, Out, Mat>
where
  Key: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  pub(crate) const fn from_source(source: Source<(Key, Out), Mat>) -> Self {
    Self { source }
  }

  /// Merges grouped substreams back into the parent source.
  #[must_use]
  pub fn merge_substreams(self) -> Source<Out, Mat> {
    self.source.map(|(_, value)| value)
  }

  /// Connects this sub-flow to a sink, merging substreams first.
  #[must_use]
  pub fn to<Mat2>(self, sink: Sink<Out, Mat2>) -> RunnableGraph<Mat> {
    self.merge_substreams().to(sink)
  }

  /// Maps each element's value within grouped substreams, preserving keys.
  #[must_use]
  pub fn map<T, F>(self, mut func: F) -> SourceGroupBySubFlow<Key, T, Mat>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> T + Send + Sync + 'static, {
    SourceGroupBySubFlow::from_source(self.source.map(move |(key, value)| (key, func(value))))
  }

  /// Filters elements within grouped substreams by value, preserving keys.
  #[must_use]
  pub fn filter<F>(self, mut predicate: F) -> SourceGroupBySubFlow<Key, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    SourceGroupBySubFlow::from_source(self.source.filter(move |(_, value)| predicate(value)))
  }
}
