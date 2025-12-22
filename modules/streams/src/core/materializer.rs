//! Materializer trait for running stream graphs.

use crate::core::{materialized::Materialized, runnable_graph::RunnableGraph, stream_error::StreamError};

/// Materializer interface.
pub trait Materializer {
  /// Handle type produced by this materializer.
  type Handle;

  /// Starts the materializer.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::AlreadyStarted` if the materializer is already running.
  fn start(&mut self) -> Result<(), StreamError>;
  /// Materializes a runnable graph.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::NotStarted` if the materializer has not been started.
  fn materialize(&mut self, graph: RunnableGraph) -> Result<Materialized<Self::Handle>, StreamError>;
  /// Shuts down the materializer.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::AlreadyShutdown` if the materializer is already stopped.
  fn shutdown(&mut self) -> Result<(), StreamError>;
}
