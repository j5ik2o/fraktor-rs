//! Runtime container responsible for executing an actor instance.

use alloc::string::String;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{dispatcher::Dispatcher, mailbox::Mailbox, pid::Pid, RuntimeToolbox};

/// Minimal actor cell structure holding runtime wiring.
pub struct ActorCell<TB: RuntimeToolbox + 'static> {
  pid:        Pid,
  parent:     Option<Pid>,
  name:       String,
  mailbox:    ArcShared<Mailbox<TB>>,
  dispatcher: Dispatcher<TB>,
}

impl<TB: RuntimeToolbox + 'static> ActorCell<TB> {
  /// Creates a new actor cell from its components.
  #[must_use]
  pub fn new(
    pid: Pid,
    parent: Option<Pid>,
    name: String,
    mailbox: ArcShared<Mailbox<TB>>,
    dispatcher: Dispatcher<TB>,
  ) -> Self {
    Self { pid, parent, name, mailbox, dispatcher }
  }

  /// Returns the pid associated with the cell.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the logical actor name.
  #[must_use]
  pub fn name(&self) -> &str {
    &self.name
  }

  /// Returns the parent pid if registered.
  #[must_use]
  pub const fn parent(&self) -> Option<Pid> {
    self.parent
  }

  /// Returns a handle to the mailbox managed by this cell.
  #[must_use]
  pub fn mailbox(&self) -> ArcShared<Mailbox<TB>> {
    self.mailbox.clone()
  }

  /// Returns the dispatcher associated with this cell.
  #[must_use]
  pub fn dispatcher(&self) -> Dispatcher<TB> {
    self.dispatcher.clone()
  }
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for ActorCell<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for ActorCell<TB> {}

#[cfg(test)]
mod tests {
  use alloc::string::ToString;

  use cellactor_utils_core_rs::sync::ArcShared;

  use crate::{
    dispatcher::Dispatcher,
    mailbox::{Mailbox, MailboxPolicy},
    pid::Pid,
    RuntimeToolbox,
  };

  use super::ActorCell;

  fn build_mailbox<TB: RuntimeToolbox + 'static>() -> ArcShared<Mailbox<TB>> {
    let policy = MailboxPolicy::unbounded(None);
    ArcShared::new(Mailbox::new(policy))
  }

  #[test]
  fn actor_cell_holds_components() {
    let mailbox = build_mailbox();
    let dispatcher = Dispatcher::with_inline_executor(mailbox.clone());
    let cell = ActorCell::new(Pid::new(1, 0), None, "worker".to_string(), mailbox.clone(), dispatcher.clone());

    assert_eq!(cell.pid(), Pid::new(1, 0));
    assert_eq!(cell.name(), "worker");
    assert!(cell.parent().is_none());
    assert_eq!(cell.mailbox().system_len(), 0);
    assert_eq!(cell.dispatcher().mailbox().system_len(), 0);
  }
}
