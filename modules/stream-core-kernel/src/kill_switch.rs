use super::StreamError;

#[cfg(test)]
#[path = "kill_switch_test.rs"]
mod tests;

/// Shared contract for externally controlled stream termination handles.
pub trait KillSwitch {
  /// Requests graceful shutdown.
  fn shutdown(&self);

  /// Requests abort with an error.
  fn abort(&self, error: StreamError);

  /// Returns true when shutdown has been requested.
  fn is_shutdown(&self) -> bool;

  /// Returns true when abort has been requested.
  fn is_aborted(&self) -> bool;

  /// Returns the abort error if the switch has been aborted.
  fn abort_error(&self) -> Option<StreamError>;
}
