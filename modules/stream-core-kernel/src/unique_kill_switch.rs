use alloc::vec::Vec;

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::{KillSwitch, StreamError};
use crate::{
  dsl::{BidiFlow, Flow},
  materialization::StreamNotUsed,
};

#[cfg(test)]
#[path = "unique_kill_switch_test.rs"]
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
    Self { state: ArcShared::new(SpinSyncMutex::new(KillSwitchState::running())) }
  }

  pub(crate) const fn from_state(state: KillSwitchStateHandle) -> Self {
    Self { state }
  }

  pub(crate) fn state_handle(&self) -> KillSwitchStateHandle {
    self.state.clone()
  }

  /// Returns a pass-through flow bound to this unique kill switch.
  #[must_use]
  pub fn flow<T>(&self) -> Flow<T, T, UniqueKillSwitch>
  where
    T: Send + Sync + 'static, {
    Flow::<T, T, StreamNotUsed>::from_kill_switch_state(self.state_handle()).map_materialized_value(|_| self.clone())
  }

  /// Returns a bidirectional pass-through flow bound to this unique kill switch.
  #[must_use]
  pub fn bidi_flow<T1, T2>(&self) -> BidiFlow<T1, T1, T2, T2, UniqueKillSwitch>
  where
    T1: Send + Sync + 'static,
    T2: Send + Sync + 'static, {
    let top = Flow::<T1, T1, StreamNotUsed>::from_kill_switch_state(self.state_handle());
    let bottom = Flow::<T2, T2, StreamNotUsed>::from_kill_switch_state(self.state_handle());
    BidiFlow::from_flows_mat(top, bottom, self.clone())
  }

  /// Requests graceful shutdown.
  pub fn shutdown(&self) {
    let command_targets = {
      let mut state = self.state.lock();
      state.request_shutdown()
    };
    if let Some(command_targets) = command_targets {
      for target in command_targets {
        if target.shutdown().is_err() {
          // Actor command delivery is best-effort because the public kill switch
          // contract has no error channel; stream polling still observes the
          // same kill switch state.
        }
      }
    }
  }

  /// Requests abort with an error.
  pub fn abort(&self, error: StreamError) {
    let abort = {
      let mut state = self.state.lock();
      state.request_abort(error)
    };
    if let Some((error, command_targets)) = abort {
      for target in command_targets {
        if target.abort(error.clone()).is_err() {
          // Actor command delivery is best-effort because the public kill switch
          // contract has no error channel; stream polling still observes the
          // same kill switch state.
        }
      }
    }
  }

  /// Returns true when the switch has been shut down.
  #[must_use]
  pub fn is_shutdown(&self) -> bool {
    matches!(self.state.lock().status(), KillSwitchStatus::Shutdown)
  }

  /// Returns true when the switch has been aborted.
  #[must_use]
  pub fn is_aborted(&self) -> bool {
    matches!(self.state.lock().status(), KillSwitchStatus::Aborted(_))
  }

  /// Returns the abort error if the switch is aborted.
  #[must_use]
  pub fn abort_error(&self) -> Option<StreamError> {
    self.state.lock().abort_error()
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

pub(crate) type KillSwitchStateHandle = ArcShared<SpinSyncMutex<KillSwitchState>>;

#[derive(Clone)]
pub(crate) struct KillSwitchState {
  status:          KillSwitchStatus,
  command_targets: Vec<KillSwitchCommandTargetShared>,
}

impl KillSwitchState {
  /// Creates a running kill switch state.
  #[must_use]
  pub(crate) fn running() -> Self {
    Self { status: KillSwitchStatus::Running, command_targets: Vec::new() }
  }

  /// Returns the lifecycle status.
  #[must_use]
  pub(crate) const fn status(&self) -> &KillSwitchStatus {
    &self.status
  }

  /// Registers an actor command target and returns the current status.
  pub(crate) fn add_command_target(&mut self, target: KillSwitchCommandTargetShared) -> KillSwitchStatus {
    self.command_targets.push(target);
    self.status.clone()
  }

  /// Removes a previously registered actor command target.
  pub(crate) fn remove_command_target(&mut self, target: &KillSwitchCommandTargetShared) -> bool {
    let Some(position) = self.command_targets.iter().position(|registered| ArcShared::ptr_eq(registered, target))
    else {
      return false;
    };
    drop(self.command_targets.remove(position));
    true
  }

  /// Moves the state to shutdown and returns registered command targets.
  pub(crate) fn request_shutdown(&mut self) -> Option<Vec<KillSwitchCommandTargetShared>> {
    if !matches!(self.status, KillSwitchStatus::Running) {
      return None;
    }
    self.status = KillSwitchStatus::Shutdown;
    Some(self.command_targets.clone())
  }

  /// Moves the state to aborted and returns the abort error with command targets.
  pub(crate) fn request_abort(
    &mut self,
    error: StreamError,
  ) -> Option<(StreamError, Vec<KillSwitchCommandTargetShared>)> {
    if matches!(self.status, KillSwitchStatus::Aborted(_)) {
      return None;
    }
    self.status = KillSwitchStatus::Aborted(error.clone());
    Some((error, self.command_targets.clone()))
  }

  /// Returns the abort error when the state is aborted.
  #[must_use]
  pub(crate) fn abort_error(&self) -> Option<StreamError> {
    match &self.status {
      | KillSwitchStatus::Aborted(error) => Some(error.clone()),
      | _ => None,
    }
  }
}

/// Shared command target used by graph kill switches.
pub(crate) type KillSwitchCommandTargetShared = ArcShared<dyn KillSwitchCommandTarget>;

/// Internal command sink notified when a graph kill switch changes state.
pub(crate) trait KillSwitchCommandTarget: Send + Sync {
  /// Sends a shutdown command to the target.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when command delivery fails.
  fn shutdown(&self) -> Result<(), StreamError>;

  /// Sends an abort command to the target.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when command delivery fails.
  fn abort(&self, error: StreamError) -> Result<(), StreamError>;
}

/// Lifecycle status stored in a kill switch state.
#[derive(Clone)]
pub(crate) enum KillSwitchStatus {
  /// The stream graph is running.
  Running,
  /// Shutdown was requested.
  Shutdown,
  /// Abort was requested with the preserved error.
  Aborted(StreamError),
}
