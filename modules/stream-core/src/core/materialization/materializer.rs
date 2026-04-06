use super::{Materialized, RunnableGraph, StreamError};

/// Stream materializer contract.
pub trait Materializer {
  /// Starts the materializer.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when startup fails.
  fn start(&mut self) -> Result<(), StreamError>;

  /// Materializes the provided graph.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when materialization fails.
  fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat>, StreamError>;

  /// Shuts down the materializer.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when shutdown fails.
  fn shutdown(&mut self) -> Result<(), StreamError>;
}
