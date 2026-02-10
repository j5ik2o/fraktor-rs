use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{
  StreamError,
  unique_kill_switch::{KillSwitchState, KillSwitchStateHandle},
};

#[cfg(test)]
mod tests;

/// Kill switch that can be shared across multiple stream instances.
#[derive(Clone)]
pub struct SharedKillSwitch {
  state: KillSwitchStateHandle,
}

impl SharedKillSwitch {
  /// Creates a new shared kill switch in running state.
  #[must_use]
  pub fn new() -> Self {
    Self { state: ArcShared::new(SpinSyncMutex::new(KillSwitchState::Running)) }
  }

  pub(super) const fn from_state(state: KillSwitchStateHandle) -> Self {
    Self { state }
  }

  pub(super) fn state_handle(&self) -> KillSwitchStateHandle {
    self.state.clone()
  }

  /// Requests graceful shutdown.
  pub fn shutdown(&self) {
    let mut state = self.state.lock();
    if !matches!(&*state, KillSwitchState::Running) {
      return;
    }
    *state = KillSwitchState::Shutdown;
  }

  /// Requests abort with an error.
  pub fn abort(&self, error: StreamError) {
    let mut state = self.state.lock();
    if !matches!(&*state, KillSwitchState::Running) {
      return;
    }
    *state = KillSwitchState::Aborted(error);
  }

  /// Returns true when the switch has been shut down.
  #[must_use]
  pub fn is_shutdown(&self) -> bool {
    matches!(*self.state.lock(), KillSwitchState::Shutdown)
  }

  /// Returns true when the switch has been aborted.
  #[must_use]
  pub fn is_aborted(&self) -> bool {
    matches!(*self.state.lock(), KillSwitchState::Aborted(_))
  }

  /// Returns the abort error if the switch is aborted.
  #[must_use]
  pub fn abort_error(&self) -> Option<StreamError> {
    match &*self.state.lock() {
      | KillSwitchState::Aborted(error) => Some(error.clone()),
      | _ => None,
    }
  }
}

impl Default for SharedKillSwitch {
  fn default() -> Self {
    Self::new()
  }
}
