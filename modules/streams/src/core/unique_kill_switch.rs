use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::StreamError;

#[cfg(test)]
mod tests;

/// Kill switch that controls a single stream instance.
pub struct UniqueKillSwitch {
  state: KillSwitchStateHandle,
}

impl UniqueKillSwitch {
  /// Creates a new unique kill switch in running state.
  #[must_use]
  pub fn new() -> Self {
    Self { state: ArcShared::new(SpinSyncMutex::new(KillSwitchState::Running)) }
  }

  pub(super) const fn from_state(state: KillSwitchStateHandle) -> Self {
    Self { state }
  }

  /// Requests graceful shutdown.
  pub fn shutdown(&self) {
    *self.state.lock() = KillSwitchState::Shutdown;
  }

  /// Requests abort with an error.
  pub fn abort(&self, error: StreamError) {
    *self.state.lock() = KillSwitchState::Aborted(error);
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

impl Default for UniqueKillSwitch {
  fn default() -> Self {
    Self::new()
  }
}

pub(super) type KillSwitchStateHandle = ArcShared<SpinSyncMutex<KillSwitchState>>;

#[derive(Clone)]
pub(super) enum KillSwitchState {
  Running,
  Shutdown,
  Aborted(StreamError),
}
