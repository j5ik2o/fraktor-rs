use alloc::{vec, vec::Vec};

#[cfg(test)]
mod tests;

use super::{
  DemandTracker, DriveOutcome, DynValue, FlowDefinition, SinkDecision, SinkDefinition, SourceDefinition, StreamBuffer,
  StreamBufferConfig, StreamError, StreamPlan, StreamState,
};

/// Executes a linear stream graph.
pub struct GraphInterpreter {
  source:        SourceDefinition,
  flows:         Vec<FlowDefinition>,
  sink:          SinkDefinition,
  demand:        DemandTracker,
  buffer:        StreamBuffer<DynValue>,
  state:         StreamState,
  source_done:   bool,
  on_start_done: bool,
}

impl GraphInterpreter {
  /// Creates a new interpreter from the provided plan.
  #[must_use]
  pub(super) fn new(plan: StreamPlan, buffer_config: StreamBufferConfig) -> Self {
    Self {
      source:        plan.source,
      flows:         plan.flows,
      sink:          plan.sink,
      demand:        DemandTracker::new(),
      buffer:        StreamBuffer::new(buffer_config),
      state:         StreamState::Idle,
      source_done:   false,
      on_start_done: false,
    }
  }

  /// Starts the interpreter.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the sink cannot start.
  pub fn start(&mut self) -> Result<(), StreamError> {
    if self.state != StreamState::Idle {
      return Ok(());
    }
    self.state = StreamState::Running;
    if !self.on_start_done {
      self.sink.logic.on_start(&mut self.demand)?;
      self.on_start_done = true;
    }
    Ok(())
  }

  /// Returns the current stream state.
  #[must_use]
  pub const fn state(&self) -> StreamState {
    self.state
  }

  /// Cancels the stream.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when cancellation cannot be processed.
  pub const fn cancel(&mut self) -> Result<(), StreamError> {
    if self.state.is_terminal() {
      return Ok(());
    }
    self.state = StreamState::Cancelled;
    Ok(())
  }

  /// Drives the interpreter once.
  #[must_use]
  pub fn drive(&mut self) -> DriveOutcome {
    if self.state != StreamState::Running {
      return DriveOutcome::Idle;
    }

    let mut progressed = false;

    if !self.on_start_done {
      match self.sink.logic.on_start(&mut self.demand) {
        | Ok(()) => {
          self.on_start_done = true;
          progressed = true;
        },
        | Err(error) => {
          self.fail(error);
          return DriveOutcome::Progressed;
        },
      }
    }

    if self.state != StreamState::Running {
      return DriveOutcome::Progressed;
    }

    if self.demand.has_demand() {
      if self.buffer.is_empty() && !self.source_done {
        match self.source.logic.pull() {
          | Ok(Some(value)) => {
            progressed = true;
            if value.as_ref().type_id() != self.source.output_type {
              self.fail(StreamError::TypeMismatch);
              return DriveOutcome::Progressed;
            }
            match self.apply_flows(value) {
              | Ok(outputs) => {
                for out in outputs {
                  if self.buffer.offer(out).is_err() {
                    self.fail(StreamError::BufferOverflow);
                    return DriveOutcome::Progressed;
                  }
                }
              },
              | Err(error) => {
                self.fail(error);
                return DriveOutcome::Progressed;
              },
            }
          },
          | Ok(None) => {
            self.source_done = true;
            progressed = true;
          },
          | Err(error) => {
            self.fail(error);
            return DriveOutcome::Progressed;
          },
        }
      }

      if self.state != StreamState::Running {
        return DriveOutcome::Progressed;
      }

      if !self.buffer.is_empty() {
        match self.buffer.poll() {
          | Ok(value) => {
            progressed = true;
            if value.as_ref().type_id() != self.sink.input_type {
              self.fail(StreamError::TypeMismatch);
              return DriveOutcome::Progressed;
            }
            if let Err(error) = self.demand.consume(1) {
              self.fail(error);
              return DriveOutcome::Progressed;
            }
            match self.sink.logic.on_push(value, &mut self.demand) {
              | Ok(SinkDecision::Continue) => {},
              | Ok(SinkDecision::Complete) => {
                if let Err(error) = self.sink.logic.on_complete() {
                  self.fail(error);
                } else {
                  self.state = StreamState::Completed;
                }
                return DriveOutcome::Progressed;
              },
              | Err(error) => {
                self.fail(error);
                return DriveOutcome::Progressed;
              },
            }
          },
          | Err(error) => {
            self.fail(error);
            return DriveOutcome::Progressed;
          },
        }
      }
    }

    if self.source_done && self.buffer.is_empty() && self.state == StreamState::Running {
      match self.sink.logic.on_complete() {
        | Ok(()) => {
          self.state = StreamState::Completed;
          progressed = true;
        },
        | Err(error) => {
          self.fail(error);
          return DriveOutcome::Progressed;
        },
      }
    }

    if progressed { DriveOutcome::Progressed } else { DriveOutcome::Idle }
  }

  fn apply_flows(&mut self, value: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let mut values = vec![value];
    for flow in &mut self.flows {
      let mut next = Vec::new();
      for value in values {
        if value.as_ref().type_id() != flow.input_type {
          return Err(StreamError::TypeMismatch);
        }
        let outputs = flow.logic.apply(value)?;
        for output in &outputs {
          if output.as_ref().type_id() != flow.output_type {
            return Err(StreamError::TypeMismatch);
          }
        }
        next.extend(outputs);
      }
      values = next;
    }
    Ok(values)
  }

  fn fail(&mut self, error: StreamError) {
    if self.state.is_terminal() {
      return;
    }
    self.state = StreamState::Failed;
    self.sink.logic.on_error(error);
  }
}
