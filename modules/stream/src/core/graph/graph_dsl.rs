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

  /// Creates a flow from a builder block.
  ///
  /// Corresponds to Pekko's `GraphDSL.create() { implicit builder => ... }`.
  /// The builder block receives a mutable reference to a fresh
  /// [`GraphDslBuilder`] and can use [`add_source`](GraphDslBuilder::add_source),
  /// [`add_flow`](GraphDslBuilder::add_flow), [`add_sink`](GraphDslBuilder::add_sink),
  /// and [`connect`](GraphDslBuilder::connect) to assemble a non-linear graph.
  #[must_use]
  pub fn create_flow<In, Out, F>(build_block: F) -> Flow<In, Out, StreamNotUsed>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
    F: FnOnce(&mut GraphDslBuilder<In, Out, StreamNotUsed>), {
    Self::create_flow_mat(StreamNotUsed::new(), build_block)
  }

  /// Creates a flow from a builder block with an initial materialized value.
  ///
  /// Corresponds to Pekko's `GraphDSL.createGraph(g1) { implicit builder => ... }`.
  #[must_use]
  pub fn create_flow_mat<In, Out, Mat, F>(mat: Mat, build_block: F) -> Flow<In, Out, Mat>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
    F: FnOnce(&mut GraphDslBuilder<In, Out, Mat>), {
    let mut builder = GraphDslBuilder::from_graph(super::StreamGraph::new(), mat);
    build_block(&mut builder);
    builder.build()
  }
}
