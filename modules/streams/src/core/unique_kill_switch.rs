use super::StreamError;

#[cfg(test)]
mod tests;

/// Kill switch that controls a single stream instance.
pub struct UniqueKillSwitch {
  state: KillSwitchState,
}

impl UniqueKillSwitch {
  /// Creates a new unique kill switch in running state.
  #[must_use]
  pub const fn new() -> Self {
    Self { state: KillSwitchState::Running }
  }

  /// Requests graceful shutdown.
  pub const fn shutdown(&mut self) {
    self.state = KillSwitchState::Shutdown;
  }

  /// Requests abort with an error.
  pub const fn abort(&mut self, error: StreamError) {
    self.state = KillSwitchState::Aborted(error);
  }

  /// Returns true when the switch has been shut down.
  #[must_use]
  pub const fn is_shutdown(&self) -> bool {
    matches!(self.state, KillSwitchState::Shutdown)
  }

  /// Returns true when the switch has been aborted.
  #[must_use]
  pub const fn is_aborted(&self) -> bool {
    matches!(self.state, KillSwitchState::Aborted(_))
  }

  /// Returns the abort error if the switch is aborted.
  #[must_use]
  pub fn abort_error(&self) -> Option<StreamError> {
    match &self.state {
      | KillSwitchState::Aborted(error) => Some(error.clone()),
      | _ => None,
    }
  }
}

impl Default for UniqueKillSwitch {
  fn default() -> Self {
    Self::new()
  }
}

enum KillSwitchState {
  Running,
  Shutdown,
  Aborted(StreamError),
}
