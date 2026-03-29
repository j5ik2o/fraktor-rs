use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{KillSwitch, StreamError};
use crate::core::{dsl::Flow, stream_not_used::StreamNotUsed};

#[cfg(test)]
mod tests;

/// Kill switch that controls a single stream instance.
#[derive(Clone)]
pub struct UniqueKillSwitch {
  state: KillSwitchStateHandle,
}

impl UniqueKillSwitch {
  /// Creates a new unique kill switch in running state.
  #[must_use]
  pub fn new() -> Self {
    Self { state: ArcShared::new(SpinSyncMutex::new(KillSwitchState::Running)) }
  }

  pub(in crate::core) const fn from_state(state: KillSwitchStateHandle) -> Self {
    Self { state }
  }

  pub(in crate::core) fn state_handle(&self) -> KillSwitchStateHandle {
    self.state.clone()
  }

  /// Returns a pass-through flow bound to this unique kill switch.
  #[must_use]
  pub fn flow<T>(&self) -> Flow<T, T, UniqueKillSwitch>
  where
    T: Send + Sync + 'static, {
    Flow::<T, T, StreamNotUsed>::from_kill_switch_state(self.state_handle()).map_materialized_value(|_| self.clone())
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

impl KillSwitch for UniqueKillSwitch {
  fn shutdown(&self) {
    UniqueKillSwitch::shutdown(self);
  }

  fn abort(&self, error: StreamError) {
    UniqueKillSwitch::abort(self, error);
  }

  fn is_shutdown(&self) -> bool {
    UniqueKillSwitch::is_shutdown(self)
  }

  fn is_aborted(&self) -> bool {
    UniqueKillSwitch::is_aborted(self)
  }

  fn abort_error(&self) -> Option<StreamError> {
    UniqueKillSwitch::abort_error(self)
  }
}

impl Default for UniqueKillSwitch {
  fn default() -> Self {
    Self::new()
  }
}

pub(in crate::core) type KillSwitchStateHandle = ArcShared<SpinSyncMutex<KillSwitchState>>;

#[derive(Clone)]
pub(in crate::core) enum KillSwitchState {
  Running,
  Shutdown,
  Aborted(StreamError),
}
