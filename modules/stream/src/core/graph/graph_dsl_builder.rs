use core::marker::PhantomData;

use super::{MatCombine, StreamError, StreamGraph, shape::{Inlet, Outlet}};
use crate::core::{
  MatCombineRule, StreamNotUsed,
  stage::{Sink, Source, flow::Flow},
};

#[cfg(test)]
mod tests;

/// Builder facade for composing stream graphs.
///
/// Supports both linear composition (via [`via`](Self::via) / [`to`](Self::to))
/// and non-linear topologies (via [`add_source`](Self::add_source) /
/// [`add_flow`](Self::add_flow) / [`add_sink`](Self::add_sink) /
/// [`connect`](Self::connect)).
pub struct GraphDslBuilder<In, Out, Mat> {
  graph: StreamGraph,
  mat:   Mat,
  _pd:   PhantomData<fn(In) -> Out>,
}

impl<T> GraphDslBuilder<T, T, StreamNotUsed> {
  /// Creates an empty builder.
  #[must_use]
  pub fn new() -> Self {
    Self { graph: StreamGraph::new(), mat: StreamNotUsed::new(), _pd: PhantomData }
  }
}

impl<T> Default for GraphDslBuilder<T, T, StreamNotUsed> {
  fn default() -> Self {
    Self::new()
  }
}

impl<In, Out, Mat> GraphDslBuilder<In, Out, Mat> {
  /// Creates a builder from a pre-built stream graph.
  #[must_use]
  pub fn from_graph(graph: StreamGraph, mat: Mat) -> Self {
    Self { graph, mat, _pd: PhantomData }
  }

  /// Creates a builder from an existing flow.
  #[must_use]
  pub fn from_flow(flow: Flow<In, Out, Mat>) -> Self {
    let (graph, mat) = flow.into_parts();
    Self::from_graph(graph, mat)
  }

  /// Maps the materialized value while keeping the graph unchanged.
  #[must_use]
  pub fn map_materialized_value<Mat2, F>(self, func: F) -> GraphDslBuilder<In, Out, Mat2>
  where
    F: FnOnce(Mat) -> Mat2, {
    let (graph, mat) = self.into_parts();
    GraphDslBuilder::from_graph(graph, func(mat))
  }

  /// Consumes the builder and returns the underlying graph and materialized value.
  #[must_use]
  pub fn into_parts(self) -> (StreamGraph, Mat) {
    (self.graph, self.mat)
  }

  /// Finalizes the builder as a flow.
  #[must_use]
  pub fn build(self) -> Flow<In, Out, Mat> {
    Flow::from_graph(self.graph, self.mat)
  }

  /// Appends a flow to this builder.
  #[must_use]
  pub fn via<T, Mat2>(self, flow: Flow<Out, T, Mat2>) -> GraphDslBuilder<In, T, Mat>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
    T: Send + Sync + 'static, {
    self.via_mat(flow, crate::core::KeepLeft)
  }

  /// Appends a flow with a custom materialized value rule.
  #[must_use]
  pub fn via_mat<T, Mat2, C>(self, flow: Flow<Out, T, Mat2>, combine: C) -> GraphDslBuilder<In, T, C::Out>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
    T: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    GraphDslBuilder::from_flow(self.build().via_mat(flow, combine))
  }

  /// Connects the builder to a sink.
  #[must_use]
  pub fn to<Mat2>(self, sink: Sink<Out, Mat2>) -> Sink<In, Mat>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static, {
    self.build().to(sink)
  }

  /// Connects the builder to a sink with a custom materialized value rule.
  #[must_use]
  pub fn to_mat<Mat2, C>(self, sink: Sink<Out, Mat2>, combine: C) -> Sink<In, C::Out>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    self.build().to_mat(sink, combine)
  }

  /// Imports a source graph and returns its outlet port.
  ///
  /// Corresponds to Pekko's `GraphDSL.Builder.add(sourceGraph)`.
  /// The source's materialized value is discarded.
  #[must_use]
  pub fn add_source<T, Mat2>(&mut self, source: Source<T, Mat2>) -> Outlet<T>
  where
    T: Send + Sync + 'static, {
    let (other_graph, _mat) = source.into_parts();
    let outlet_id = other_graph.tail_outlet().expect("source graph must have an outlet");
    self.graph.append_unwired(other_graph);
    Outlet::from_id(outlet_id)
  }

  /// Imports a flow graph and returns its (inlet, outlet) port pair.
  ///
  /// Corresponds to Pekko's `GraphDSL.Builder.add(flowGraph)`.
  /// The flow's materialized value is discarded.
  #[must_use]
  pub fn add_flow<I, O, Mat2>(&mut self, flow: Flow<I, O, Mat2>) -> (Inlet<I>, Outlet<O>)
  where
    I: Send + Sync + 'static,
    O: Send + Sync + 'static, {
    let (other_graph, _mat) = flow.into_parts();
    let inlet_id = other_graph.head_inlet().expect("flow graph must have an inlet");
    let outlet_id = other_graph.tail_outlet().expect("flow graph must have an outlet");
    self.graph.append_unwired(other_graph);
    (Inlet::from_id(inlet_id), Outlet::from_id(outlet_id))
  }

  /// Imports a sink graph and returns its inlet port.
  ///
  /// Corresponds to Pekko's `GraphDSL.Builder.add(sinkGraph)`.
  /// The sink's materialized value is discarded.
  #[must_use]
  pub fn add_sink<T, Mat2>(&mut self, sink: Sink<T, Mat2>) -> Inlet<T>
  where
    T: Send + Sync + 'static, {
    let (other_graph, _mat) = sink.into_parts();
    let inlet_id = other_graph.head_inlet().expect("sink graph must have an inlet");
    self.graph.append_unwired(other_graph);
    Inlet::from_id(inlet_id)
  }

  /// Connects an outlet to an inlet within this builder's graph.
  ///
  /// Corresponds to Pekko's `~>` operator.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] when a port is unknown.
  pub fn connect<T>(&mut self, from: &Outlet<T>, to: &Inlet<T>) -> Result<(), StreamError> {
    self.graph.connect(from, to, MatCombine::KeepLeft)
  }
}
