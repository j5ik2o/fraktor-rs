//! Running state wrapper providing non-Option guardian accessors.

#![allow(dead_code)]

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  actor::{ActorCellGeneric, Pid},
  system::{GuardianKind, SystemStateSharedGeneric},
};

/// Wrapper for the system state after all guardians are registered.
pub(crate) struct RunningSystemStateGeneric<TB: RuntimeToolbox + 'static> {
  state: SystemStateSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> RunningSystemStateGeneric<TB> {
  pub(crate) const fn new(state: SystemStateSharedGeneric<TB>) -> Self {
    Self { state }
  }

  pub(crate) fn guardian_pid(&self, kind: GuardianKind) -> Pid {
    self.state.guardian_pid(kind).unwrap_or_else(|| panic!("guardian pid must be set in running state"))
  }

  pub(crate) fn guardian_cell(&self, kind: GuardianKind) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    let pid = self.guardian_pid(kind);
    self.state.cell(&pid)
  }
}
