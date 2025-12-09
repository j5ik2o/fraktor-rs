//! Booting state wrapper that enforces guardian registration before running.

#![allow(dead_code)]

use crate::core::{
  actor_prim::Pid,
  spawn::SpawnError,
  system::{GuardianKind, SystemStateSharedGeneric, running_state::RunningSystemStateGeneric},
};

/// Wrapper for the system state while guardians are being registered.
pub(crate) struct BootingSystemStateGeneric<TB: fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox + 'static> {
  state: SystemStateSharedGeneric<TB>,
}

impl<TB: fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox + 'static> BootingSystemStateGeneric<TB> {
  pub(crate) const fn new(state: SystemStateSharedGeneric<TB>) -> Self {
    Self { state }
  }

  pub(crate) fn register_guardian(&self, kind: GuardianKind, pid: Pid) {
    self.state.register_guardian_pid(kind, pid);
  }

  pub(crate) fn into_running(self) -> Result<RunningSystemStateGeneric<TB>, SpawnError> {
    let missing = [
      (GuardianKind::Root, self.state.root_guardian_pid()),
      (GuardianKind::System, self.state.system_guardian_pid()),
      (GuardianKind::User, self.state.user_guardian_pid()),
    ]
    .iter()
    .find_map(|(kind, pid)| if pid.is_none() { Some(*kind) } else { None });

    if missing.is_some() {
      return Err(SpawnError::system_not_bootstrapped());
    }

    Ok(RunningSystemStateGeneric::new(self.state))
  }
}
