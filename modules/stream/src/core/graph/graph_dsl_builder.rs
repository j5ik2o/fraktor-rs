use core::marker::PhantomData;

use super::StreamGraph;
use crate::core::{
  MatCombineRule, StreamNotUsed,
  stage::{Sink, flow::Flow},
};

#[cfg(test)]
mod tests;

/// Minimal builder facade for composing stream graphs.
///
/// This builder intentionally reuses the existing linear `Flow` composition
/// model instead of introducing arbitrary port wiring.
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
}
