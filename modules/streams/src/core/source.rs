//! Source stage definition.

use crate::core::{
  flow::Flow, inlet_id::InletId, mat_combine::MatCombine, outlet_id::OutletId, runnable_graph::RunnableGraph,
  sink::Sink, stage_id::StageId, stream_builder::StreamBuilder, stream_error::StreamError, stream_graph::StreamGraph,
  stream_shape::StreamShape, stream_stage::StreamStage,
};

/// Stream source stage.
#[derive(Debug, Clone)]
pub struct Source<T> {
  stage:  StageId,
  outlet: OutletId<T>,
}

impl<T> Source<T> {
  /// Creates a new source stage.
  #[must_use]
  pub fn new() -> Self {
    let stage = StageId::next();
    let outlet = OutletId::new(stage);
    Self { stage, outlet }
  }

  /// Returns the outlet port identifier.
  #[must_use]
  pub const fn outlet(&self) -> OutletId<T> {
    self.outlet
  }

  /// Returns the stage identifier.
  #[must_use]
  pub const fn stage_id(&self) -> StageId {
    self.stage
  }

  /// Attaches a flow stage and returns a builder for chaining.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::InvalidConnection` when the connection is invalid.
  pub fn via<Out>(self, flow: &Flow<T, Out>, combine: MatCombine) -> Result<StreamBuilder<Out>, StreamError> {
    let mut graph = StreamGraph::new();
    graph.connect(self.outlet, flow.inlet(), combine)?;
    Ok(StreamBuilder::new(graph, flow.outlet()))
  }

  /// Attaches a sink stage and builds a runnable graph.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::InvalidConnection` when the connection is invalid.
  pub fn to(self, sink: &Sink<T>, combine: MatCombine) -> Result<RunnableGraph, StreamError> {
    let mut graph = StreamGraph::new();
    graph.connect(self.outlet, sink.inlet(), combine)?;
    graph.build()
  }
}

impl<T> StreamStage for Source<T> {
  type In = ();
  type Out = T;

  fn shape(&self) -> StreamShape {
    StreamShape::Source
  }

  fn inlet(&self) -> Option<InletId<Self::In>> {
    None
  }

  fn outlet(&self) -> Option<OutletId<Self::Out>> {
    Some(self.outlet)
  }
}

impl<T> Default for Source<T> {
  fn default() -> Self {
    Self::new()
  }
}
