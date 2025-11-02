//! Coordinates actors and infrastructure.

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  actor_cell::ActorCell,
  mailbox::Mailbox,
  pid::Pid,
  props::Props,
  RuntimeToolbox,
  SystemState,
};

/// Minimal actor system wiring around [`SystemState`].
pub struct ActorSystem<TB: RuntimeToolbox + 'static> {
  state: ArcShared<SystemState<TB>>,
}

impl<TB: RuntimeToolbox + 'static> ActorSystem<TB> {
  /// Creates a new actor system instance.
  #[must_use]
  pub fn new() -> Self {
    Self { state: ArcShared::new(SystemState::new()) }
  }

  /// Returns the shared system state.
  #[must_use]
  pub fn state(&self) -> ArcShared<SystemState<TB>> {
    self.state.clone()
  }

  /// Allocates a new pid.
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    self.state.allocate_pid()
  }

  /// Registers a freshly created actor cell.
  pub fn register_cell(&self, cell: ArcShared<ActorCell<TB>>) {
    self.state.register_cell(cell);
  }

  /// Retrieves a cell by pid.
  #[must_use]
  pub fn cell(&self, pid: &Pid) -> Option<ArcShared<ActorCell<TB>>> {
    self.state.cell(pid)
  }

  /// Returns the default mailbox for props.
  #[must_use]
  pub fn create_mailbox(&self, props: &Props) -> ArcShared<Mailbox<TB>> {
    let policy = props.mailbox_policy();
    ArcShared::new(Mailbox::new(policy))
  }
}

impl<TB: RuntimeToolbox + 'static> Default for ActorSystem<TB> {
  fn default() -> Self {
    Self::new()
  }
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for ActorSystem<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for ActorSystem<TB> {}

#[cfg(test)]
mod tests {
  use alloc::string::ToString;

  use cellactor_utils_core_rs::sync::ArcShared;

  use crate::{
    actor_cell::ActorCell,
    dispatcher::Dispatcher,
    mailbox::{Mailbox, MailboxPolicy},
  };

  use super::ActorSystem;

  #[test]
  fn allocates_unique_pids() {
    let system: ActorSystem<crate::NoStdToolbox> = ActorSystem::new();
    let pid1 = system.allocate_pid();
    let pid2 = system.allocate_pid();
    assert_ne!(pid1, pid2);
  }

  #[test]
  fn registers_cells() {
    let system: ActorSystem<crate::NoStdToolbox> = ActorSystem::new();
    let mailbox = ArcShared::new(Mailbox::new(MailboxPolicy::unbounded(None)));
    let dispatcher = Dispatcher::with_inline_executor(mailbox.clone());
    let cell = ArcShared::new(ActorCell::new(system.allocate_pid(), None, "root".to_string(), mailbox, dispatcher));
    let pid = cell.pid();
    system.register_cell(cell.clone());
    assert!(system.cell(&pid).is_some());
  }
}
