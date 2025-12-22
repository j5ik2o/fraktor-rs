//! Core materializer implementation.

#[cfg(test)]
mod tests;

use crate::core::{
  materialized::Materialized, materializer::Materializer, runnable_graph::RunnableGraph, stream_error::StreamError,
  stream_handle_state::StreamHandleState,
};

/// Basic materializer for manual driving.
#[derive(Debug, Default)]
pub struct StreamMaterializer {
  started: bool,
}

impl StreamMaterializer {
  /// Creates a new materializer instance.
  #[must_use]
  pub const fn new() -> Self {
    Self { started: false }
  }
}

impl Materializer for StreamMaterializer {
  type Handle = StreamHandleState;

  fn start(&mut self) -> Result<(), StreamError> {
    if self.started {
      return Err(StreamError::AlreadyStarted);
    }
    self.started = true;
    Ok(())
  }

  fn materialize(&mut self, graph: RunnableGraph) -> Result<Materialized<Self::Handle>, StreamError> {
    if !self.started {
      return Err(StreamError::NotStarted);
    }
    let handle = StreamHandleState::running();
    Ok(Materialized::new(handle, graph.materialized_value()))
  }

  fn shutdown(&mut self) -> Result<(), StreamError> {
    if !self.started {
      return Err(StreamError::AlreadyShutdown);
    }
    self.started = false;
    Ok(())
  }
}
