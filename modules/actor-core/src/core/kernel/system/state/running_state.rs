//! Running state wrapper providing non-Option guardian accessors.

#![allow(dead_code)]

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::kernel::{
  actor::{ActorCell, Pid},
  system::{guardian::GuardianKind, state::SystemStateShared},
};

/// Wrapper for the system state after all guardians are registered.
pub(crate) struct RunningSystemState {
  state: SystemStateShared,
}

impl RunningSystemState {
  pub(crate) const fn new(state: SystemStateShared) -> Self {
    Self { state }
  }

  pub(crate) fn guardian_pid(&self, kind: GuardianKind) -> Pid {
    self.state.guardian_pid(kind).unwrap_or_else(|| panic!("guardian pid must be set in running state"))
  }

  pub(crate) fn guardian_cell(&self, kind: GuardianKind) -> Option<ArcShared<ActorCell>> {
    let pid = self.guardian_pid(kind);
    self.state.cell(&pid)
  }
}
