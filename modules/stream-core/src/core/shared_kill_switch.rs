use alloc::string::String;

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::{
  KillSwitch, StreamError,
  unique_kill_switch::{KillSwitchState, KillSwitchStateHandle},
};
use crate::core::{
  attributes::Attributes,
  dsl::{BidiFlow, Flow},
  materialization::StreamNotUsed,
};

#[cfg(test)]
mod tests;

/// Kill switch that can be shared across multiple stream instances.
#[derive(Clone)]
pub struct SharedKillSwitch {
  state: KillSwitchStateHandle,
  name:  Option<String>,
}

impl SharedKillSwitch {
  /// Creates a new shared kill switch in running state.
  #[must_use]
  pub fn new() -> Self {
    Self { state: ArcShared::new(SpinSyncMutex::new(KillSwitchState::Running)), name: None }
  }

  /// Creates a new shared kill switch with a debug name.
  #[must_use]
  pub fn new_named(name: impl Into<String>) -> Self {
    Self { state: ArcShared::new(SpinSyncMutex::new(KillSwitchState::Running)), name: Some(name.into()) }
  }

  pub(in crate::core) const fn from_state(state: KillSwitchStateHandle) -> Self {
    Self { state, name: None }
  }

  pub(in crate::core) fn state_handle(&self) -> KillSwitchStateHandle {
    self.state.clone()
  }

  /// Returns the configured switch name when present.
  #[must_use]
  pub fn name(&self) -> Option<&str> {
    self.name.as_deref()
  }

  /// Returns a pass-through flow bound to this shared kill switch.
  #[must_use]
  pub fn flow<T>(&self) -> Flow<T, T, SharedKillSwitch>
  where
    T: Send + Sync + 'static, {
    let flow =
      Flow::<T, T, StreamNotUsed>::from_kill_switch_state(self.state_handle()).map_materialized_value(|_| self.clone());
    match self.name.as_deref() {
      | Some(name) => flow.add_attributes(Attributes::named(name)),
      | None => flow,
    }
  }

  /// Returns a bidirectional pass-through flow bound to this shared kill switch.
  ///
  /// When the switch carries a debug name, that name is attached as an `Attributes::named`
  /// attribute on both the top and bottom flow fragments, but not on the combined BidiFlow
  /// itself.
  #[must_use]
  pub fn bidi_flow<T1, T2>(&self) -> BidiFlow<T1, T1, T2, T2, SharedKillSwitch>
  where
    T1: Send + Sync + 'static,
    T2: Send + Sync + 'static, {
    let top = Flow::<T1, T1, StreamNotUsed>::from_kill_switch_state(self.state_handle());
    let bottom = Flow::<T2, T2, StreamNotUsed>::from_kill_switch_state(self.state_handle());
    let (top, bottom) = match self.name.as_deref() {
      | Some(name) => (top.add_attributes(Attributes::named(name)), bottom.add_attributes(Attributes::named(name))),
      | None => (top, bottom),
    };
    BidiFlow::from_flows_mat(top, bottom, self.clone())
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

impl KillSwitch for SharedKillSwitch {
  fn shutdown(&self) {
    SharedKillSwitch::shutdown(self);
  }

  fn abort(&self, error: StreamError) {
    SharedKillSwitch::abort(self, error);
  }

  fn is_shutdown(&self) -> bool {
    SharedKillSwitch::is_shutdown(self)
  }

  fn is_aborted(&self) -> bool {
    SharedKillSwitch::is_aborted(self)
  }

  fn abort_error(&self) -> Option<StreamError> {
    SharedKillSwitch::abort_error(self)
  }
}

impl Default for SharedKillSwitch {
  fn default() -> Self {
    Self::new()
  }
}
