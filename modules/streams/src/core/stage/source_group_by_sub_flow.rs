use super::Source;

#[cfg(test)]
mod tests;

/// Substream surface returned by `group_by`.
pub struct SourceGroupBySubFlow<Key, Out, Mat> {
  source: super::Source<(Key, Out), Mat>,
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
}
