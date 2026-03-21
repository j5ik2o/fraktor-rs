use super::GraphDslBuilder;
use crate::core::{StreamNotUsed, stage::flow::Flow};

/// Minimal namespace facade compatible with Pekko-style `GraphDSL`.
pub struct GraphDsl;

impl GraphDsl {
  /// Creates a builder for a linear graph fragment.
  #[must_use]
  pub fn builder<T>() -> GraphDslBuilder<T, T, StreamNotUsed> {
    GraphDslBuilder::new()
  }

  /// Wraps an existing flow into a graph builder.
  #[must_use]
  pub fn from_flow<In, Out, Mat>(flow: Flow<In, Out, Mat>) -> GraphDslBuilder<In, Out, Mat> {
    GraphDslBuilder::from_flow(flow)
  }
}
