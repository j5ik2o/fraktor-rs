//! Guardian slots tracking PIDs and liveness flags.

#[cfg(test)]
mod tests;

use crate::core::{actor_prim::Pid, system::GuardianKind};

/// Guardian slot state.
enum GuardianSlotState {
  Unset,
  Set(GuardianSlot),
}

/// Guardian slot carrying PID and liveness flag.
struct GuardianSlot {
  pid:   Pid,
  alive: bool,
}

impl GuardianSlot {
  pub(crate) const fn new(pid: Pid) -> Self {
    Self { pid, alive: true }
  }

  pub(crate) const fn set(&mut self, pid: Pid) {
    self.pid = pid;
    self.alive = true;
  }

  pub(crate) const fn clear(&mut self) {
    self.alive = false;
  }

  pub(crate) const fn pid(&self) -> Pid {
    self.pid
  }

  pub(crate) const fn is_alive(&self) -> bool {
    self.alive
  }
}

impl GuardianSlotState {
  pub(crate) const fn pid(&self) -> Option<Pid> {
    match self {
      | Self::Unset => None,
      | Self::Set(slot) => Some(slot.pid()),
    }
  }

  pub(crate) const fn is_alive(&self) -> bool {
    match self {
      | Self::Unset => false,
      | Self::Set(slot) => slot.is_alive(),
    }
  }
}

/// Aggregates guardian PID handles and liveness.
pub(crate) struct GuardiansState {
  root:   GuardianSlotState,
  system: GuardianSlotState,
  user:   GuardianSlotState,
}

impl GuardiansState {
  pub(crate) const fn new() -> Self {
    Self { root: GuardianSlotState::Unset, system: GuardianSlotState::Unset, user: GuardianSlotState::Unset }
  }

  pub(crate) const fn register(&mut self, kind: GuardianKind, pid: Pid) {
    let slot = match kind {
      | GuardianKind::Root => &mut self.root,
      | GuardianKind::System => &mut self.system,
      | GuardianKind::User => &mut self.user,
    };

    match slot {
      | GuardianSlotState::Unset => *slot = GuardianSlotState::Set(GuardianSlot::new(pid)),
      | GuardianSlotState::Set(existing) => existing.set(pid),
    }
  }

  pub(crate) fn clear_by_pid(&mut self, pid: Pid) -> Option<GuardianKind> {
    if Self::matches_and_clear(&mut self.root, pid) {
      return Some(GuardianKind::Root);
    }
    if Self::matches_and_clear(&mut self.system, pid) {
      return Some(GuardianKind::System);
    }
    if Self::matches_and_clear(&mut self.user, pid) {
      return Some(GuardianKind::User);
    }
    None
  }

  fn matches_and_clear(slot: &mut GuardianSlotState, pid: Pid) -> bool {
    match slot {
      | GuardianSlotState::Unset => false,
      | GuardianSlotState::Set(s) if s.pid == pid => {
        s.clear();
        true
      },
      | _ => false,
    }
  }

  pub(crate) const fn pid(&self, kind: GuardianKind) -> Option<Pid> {
    match kind {
      | GuardianKind::Root => self.root.pid(),
      | GuardianKind::System => self.system.pid(),
      | GuardianKind::User => self.user.pid(),
    }
  }

  pub(crate) const fn is_alive(&self, kind: GuardianKind) -> bool {
    match kind {
      | GuardianKind::Root => self.root.is_alive(),
      | GuardianKind::System => self.system.is_alive(),
      | GuardianKind::User => self.user.is_alive(),
    }
  }
}

impl Default for GuardiansState {
  fn default() -> Self {
    Self::new()
  }
}
