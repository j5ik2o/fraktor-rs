//! Guardian slots tracking PIDs.

#[cfg(test)]
mod tests;

use super::guardian_kind::GuardianKind;
use crate::core::actor::Pid;

/// Aggregates guardian PID handles.
pub(crate) struct GuardiansState {
  root:   Option<Pid>,
  system: Option<Pid>,
  user:   Option<Pid>,
}

impl GuardiansState {
  pub(crate) const fn new() -> Self {
    Self { root: None, system: None, user: None }
  }

  pub(crate) const fn register(&mut self, kind: GuardianKind, pid: Pid) {
    match kind {
      | GuardianKind::Root => self.root = Some(pid),
      | GuardianKind::System => self.system = Some(pid),
      | GuardianKind::User => self.user = Some(pid),
    }
  }

  pub(crate) const fn pid(&self, kind: GuardianKind) -> Option<Pid> {
    match kind {
      | GuardianKind::Root => self.root,
      | GuardianKind::System => self.system,
      | GuardianKind::User => self.user,
    }
  }

  pub(crate) fn kind_by_pid(&self, pid: Pid) -> Option<GuardianKind> {
    if matches!(self.root, Some(root) if root == pid) {
      return Some(GuardianKind::Root);
    }
    if matches!(self.system, Some(system) if system == pid) {
      return Some(GuardianKind::System);
    }
    if matches!(self.user, Some(user) if user == pid) {
      return Some(GuardianKind::User);
    }
    None
  }
}

impl Default for GuardiansState {
  fn default() -> Self {
    Self::new()
  }
}
