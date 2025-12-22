//! Stream DSL builder.

#[cfg(test)]
mod tests;

use crate::core::{
  flow::Flow, mat_combine::MatCombine, outlet_id::OutletId, runnable_graph::RunnableGraph, sink::Sink,
  stream_error::StreamError, stream_graph::StreamGraph,
};

/// Fluent builder for connecting stages from a source outlet.
#[derive(Debug)]
pub struct StreamBuilder<T> {
  graph:  StreamGraph,
  outlet: OutletId<T>,
}

impl<T> StreamBuilder<T> {
  pub(crate) const fn new(graph: StreamGraph, outlet: OutletId<T>) -> Self {
    Self { graph, outlet }
  }

  /// Attaches a flow stage and returns a builder for the next outlet.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::InvalidConnection` when the connection is invalid.
  pub fn via<Out>(self, flow: &Flow<T, Out>, combine: MatCombine) -> Result<StreamBuilder<Out>, StreamError> {
    let mut graph = self.graph;
    graph.connect(self.outlet, flow.inlet(), combine)?;
    Ok(StreamBuilder { graph, outlet: flow.outlet() })
  }

  /// Attaches a sink stage and builds a runnable graph.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::InvalidConnection` when the connection is invalid.
  pub fn to(self, sink: &Sink<T>, combine: MatCombine) -> Result<RunnableGraph, StreamError> {
    let mut graph = self.graph;
    graph.connect(self.outlet, sink.inlet(), combine)?;
    graph.build()
  }
}
