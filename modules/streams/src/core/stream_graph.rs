//! Stream graph composition and connection tracking.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use crate::core::{
  inlet_id::InletId, mat_combine::MatCombine, outlet_id::OutletId, runnable_graph::RunnableGraph,
  stream_error::StreamError,
};

/// Stream graph builder that tracks connections.
#[derive(Debug, Default)]
pub struct StreamGraph {
  connections:  Vec<(u64, u64, MatCombine)>,
  last_combine: MatCombine,
}

impl StreamGraph {
  /// Creates a new empty stream graph.
  #[must_use]
  pub const fn new() -> Self {
    Self { connections: Vec::new(), last_combine: MatCombine::default_rule() }
  }

  /// Connects an outlet to an inlet with the provided materialization rule.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::InvalidConnection` when the connection is duplicate or self-referential.
  ///
  /// ```compile_fail
  /// use fraktor_streams_rs::core::{mat_combine::MatCombine, sink::Sink, source::Source, stream_graph::StreamGraph};
  ///
  /// let source = Source::<u8>::new();
  /// let sink = Sink::<u16>::new();
  /// let mut graph = StreamGraph::new();
  /// graph.connect(source.outlet(), sink.inlet(), MatCombine::KeepLeft).unwrap();
  /// ```
  pub fn connect<T>(
    &mut self,
    upstream: OutletId<T>,
    downstream: InletId<T>,
    combine: MatCombine,
  ) -> Result<(), StreamError> {
    let upstream_token = upstream.token();
    let downstream_token = downstream.token();
    if upstream_token == downstream_token {
      return Err(StreamError::InvalidConnection);
    }
    if self.connections.iter().any(|(up, down, _)| *up == upstream_token && *down == downstream_token) {
      return Err(StreamError::InvalidConnection);
    }
    self.connections.push((upstream_token, downstream_token, combine));
    self.last_combine = combine;
    Ok(())
  }

  /// Returns the number of connections registered in this graph.
  #[must_use]
  pub const fn connection_count(&self) -> usize {
    self.connections.len()
  }

  /// Builds a runnable graph from the collected connections.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::InvalidConnection` when the graph has no connections.
  pub fn build(self) -> Result<RunnableGraph, StreamError> {
    if self.connections.is_empty() {
      return Err(StreamError::InvalidConnection);
    }
    Ok(RunnableGraph::new(self.connections, self.last_combine))
  }
}
