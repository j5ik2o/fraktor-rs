use super::{DriveOutcome, GraphInterpreter, StreamBufferConfig, StreamError, StreamPlan, StreamState};

/// Internal stream execution state.
pub(crate) struct Stream {
  interpreter: GraphInterpreter,
}

impl Stream {
  pub(super) fn new(plan: StreamPlan, buffer_config: StreamBufferConfig) -> Self {
    Self { interpreter: GraphInterpreter::new(plan, buffer_config) }
  }

  pub(crate) fn start(&mut self) -> Result<(), StreamError> {
    self.interpreter.start()
  }

  pub(crate) const fn state(&self) -> StreamState {
    self.interpreter.state()
  }

  pub(crate) fn drive(&mut self) -> DriveOutcome {
    self.interpreter.drive()
  }

  pub(crate) const fn cancel(&mut self) -> Result<(), StreamError> {
    self.interpreter.cancel()
  }
}
