use super::flow::Flow;

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
}
