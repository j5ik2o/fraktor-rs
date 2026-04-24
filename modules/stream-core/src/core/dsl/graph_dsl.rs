use super::{Flow, Sink, Source, StreamGraph, StreamNotUsed, graph_dsl_builder::GraphDslBuilder};

#[cfg(test)]
mod tests;

/// Namespace for building explicit stream graphs.
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

  /// Creates a flow from a builder block.
  #[must_use]
  pub fn create_flow<In, Out, F>(build_block: F) -> Flow<In, Out, StreamNotUsed>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
    F: FnOnce(&mut GraphDslBuilder<In, Out, StreamNotUsed>), {
    Self::create_flow_mat(StreamNotUsed::new(), build_block)
  }

  /// Creates a flow from a builder block with an initial materialized value.
  #[must_use]
  pub fn create_flow_mat<In, Out, Mat, F>(mat: Mat, build_block: F) -> Flow<In, Out, Mat>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
    F: FnOnce(&mut GraphDslBuilder<In, Out, Mat>), {
    let mut builder = GraphDslBuilder::from_graph(StreamGraph::new(), mat);
    build_block(&mut builder);
    builder.build()
  }

  /// Creates a source from a builder block.
  #[must_use]
  pub fn create_source<Out, F>(build_block: F) -> Source<Out, StreamNotUsed>
  where
    Out: Send + Sync + 'static,
    F: FnOnce(&mut GraphDslBuilder<(), Out, StreamNotUsed>), {
    let mut builder = GraphDslBuilder::from_graph(StreamGraph::new(), StreamNotUsed::new());
    build_block(&mut builder);
    builder.into_source()
  }

  /// Creates a sink from a builder block.
  #[must_use]
  pub fn create_sink<In, F>(build_block: F) -> Sink<In, StreamNotUsed>
  where
    In: Send + Sync + 'static,
    F: FnOnce(&mut GraphDslBuilder<In, (), StreamNotUsed>), {
    let mut builder = GraphDslBuilder::from_graph(StreamGraph::new(), StreamNotUsed::new());
    build_block(&mut builder);
    builder.into_sink()
  }
}
