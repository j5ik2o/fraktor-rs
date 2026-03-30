#![allow(dead_code)]

use super::graph_dsl_builder::GraphDslBuilder;
use crate::core::{
  dsl::{Flow, Sink, Source},
  materialization::StreamNotUsed,
};

/// Minimal namespace facade compatible with Pekko-style `GraphDSL`.
pub(crate) struct GraphDsl;

impl GraphDsl {
  /// Creates a builder for a linear graph fragment.
  #[must_use]
  pub(crate) fn builder<T>() -> GraphDslBuilder<T, T, StreamNotUsed> {
    GraphDslBuilder::new()
  }

  /// Wraps an existing flow into a graph builder.
  #[must_use]
  pub(crate) fn from_flow<In, Out, Mat>(flow: Flow<In, Out, Mat>) -> GraphDslBuilder<In, Out, Mat> {
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
  pub(crate) fn create_flow<In, Out, F>(build_block: F) -> Flow<In, Out, StreamNotUsed>
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
  pub(crate) fn create_flow_mat<In, Out, Mat, F>(mat: Mat, build_block: F) -> Flow<In, Out, Mat>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
    F: FnOnce(&mut GraphDslBuilder<In, Out, Mat>), {
    let mut builder = GraphDslBuilder::from_graph(super::stream_graph::StreamGraph::new(), mat);
    build_block(&mut builder);
    builder.build()
  }

  /// Creates a source from a builder block.
  ///
  /// Corresponds to Pekko's `GraphDSL.create() { implicit builder => ... }` for source graphs.
  /// The builder block receives a mutable reference to a fresh [`GraphDslBuilder`]
  /// and can use [`add_source`](GraphDslBuilder::add_source),
  /// [`add_flow`](GraphDslBuilder::add_flow), and [`connect`](GraphDslBuilder::connect)
  /// to assemble a source graph.
  #[must_use]
  pub(crate) fn create_source<Out, F>(build_block: F) -> Source<Out, StreamNotUsed>
  where
    Out: Send + Sync + 'static,
    F: FnOnce(&mut GraphDslBuilder<(), Out, StreamNotUsed>), {
    let mut builder = GraphDslBuilder::from_graph(super::stream_graph::StreamGraph::new(), StreamNotUsed::new());
    build_block(&mut builder);
    let (graph, mat) = builder.into_parts();
    Source::from_graph(graph, mat)
  }

  /// Creates a sink from a builder block.
  ///
  /// Corresponds to Pekko's `GraphDSL.create() { implicit builder => ... }` for sink graphs.
  /// The builder block receives a mutable reference to a fresh [`GraphDslBuilder`]
  /// and can use [`add_sink`](GraphDslBuilder::add_sink),
  /// [`add_flow`](GraphDslBuilder::add_flow), and [`connect`](GraphDslBuilder::connect)
  /// to assemble a sink graph.
  #[must_use]
  pub(crate) fn create_sink<In, F>(build_block: F) -> Sink<In, StreamNotUsed>
  where
    In: Send + Sync + 'static,
    F: FnOnce(&mut GraphDslBuilder<In, (), StreamNotUsed>), {
    let mut builder = GraphDslBuilder::from_graph(super::stream_graph::StreamGraph::new(), StreamNotUsed::new());
    build_block(&mut builder);
    let (graph, mat) = builder.into_parts();
    Sink::from_graph(graph, mat)
  }
}
