//! Stream handle implementation.

#[cfg(test)]
mod tests;

use crate::core::{
  demand_tracker::DemandTracker, drive_outcome::DriveOutcome, stream_error::StreamError, stream_state::StreamState,
};

/// Stream handle state holder.
#[derive(Debug, Clone)]
pub struct StreamHandleState {
  state:  StreamState,
  demand: DemandTracker,
}

impl StreamHandleState {
  /// Creates a new handle in idle state.
  #[must_use]
  pub const fn new() -> Self {
    Self { state: StreamState::Idle, demand: DemandTracker::new() }
  }

  /// Creates a new handle in running state.
  #[must_use]
  pub const fn running() -> Self {
    Self { state: StreamState::Running, demand: DemandTracker::new() }
  }

  /// Returns the current state.
  #[must_use]
  pub const fn state(&self) -> StreamState {
    self.state
  }

  /// Requests demand for this handle.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::InvalidDemand` when `amount` is zero.
  pub fn request(&mut self, amount: u64) -> Result<(), StreamError> {
    self.demand.request(amount)?;
    Ok(())
  }

  /// Marks the stream as completed.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::NotRunning` when the stream is not running.
  pub fn complete(&mut self) -> Result<(), StreamError> {
    if self.state != StreamState::Running {
      return Err(StreamError::NotRunning);
    }
    self.state = StreamState::Completed;
    Ok(())
  }

  /// Marks the stream as failed.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::NotRunning` when the stream is not running.
  pub fn fail(&mut self) -> Result<(), StreamError> {
    if self.state != StreamState::Running {
      return Err(StreamError::NotRunning);
    }
    self.state = StreamState::Failed;
    Ok(())
  }

  /// Cancels the stream.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::NotRunning` when the stream is not running.
  pub fn cancel(&mut self) -> Result<(), StreamError> {
    if self.state != StreamState::Running {
      return Err(StreamError::NotRunning);
    }
    self.state = StreamState::Cancelled;
    Ok(())
  }

  /// Advances the stream execution by one step.
  pub fn drive(&mut self) -> DriveOutcome {
    if self.state != StreamState::Running {
      return DriveOutcome::Idle;
    }
    if self.demand.consume_one() { DriveOutcome::Progressed } else { DriveOutcome::Idle }
  }
}

impl Default for StreamHandleState {
  fn default() -> Self {
    Self::new()
  }
}
