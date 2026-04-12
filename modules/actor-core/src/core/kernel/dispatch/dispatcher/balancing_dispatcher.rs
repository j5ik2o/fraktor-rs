//! `MessageDispatcher` that load-balances actors over a shared message queue.
//!
//! `BalancingDispatcher` carries a single [`SharedMessageQueue`] that all
//! attached actors share. The Pekko equivalent is
//! `org.apache.pekko.dispatch.BalancingDispatcher`.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, vec::Vec};

use fraktor_utils_core_rs::core::sync::{ArcShared, WeakShared};

use super::{
  dispatcher_core::DispatcherCore, dispatcher_settings::DispatcherSettings, executor_shared::ExecutorShared,
  message_dispatcher::MessageDispatcher, shared_message_queue::SharedMessageQueue,
};
use crate::core::kernel::{
  actor::{ActorCell, Pid, error::SendError, messaging::system_message::SystemMessage, spawn::SpawnError},
  dispatch::mailbox::{Envelope, Mailbox, MailboxPolicy, MessageQueue},
  system::shared_factory::MailboxSharedSetFactory,
};

/// Dispatcher that load-balances actors over a shared message queue.
pub struct BalancingDispatcher {
  core: DispatcherCore,
  shared_queue: SharedMessageQueue,
  team: Vec<WeakShared<ActorCell>>,
  mailbox_shared_set_factory: ArcShared<dyn MailboxSharedSetFactory>,
}

impl BalancingDispatcher {
  /// Constructs a new `BalancingDispatcher`.
  ///
  /// The supplied [`SharedMessageQueue`] is reused by every actor that
  /// attaches via
  /// [`MessageDispatcher::try_create_shared_mailbox`].
  #[must_use]
  pub fn new(
    settings: &DispatcherSettings,
    executor: ExecutorShared,
    shared_queue: SharedMessageQueue,
    mailbox_shared_set_factory: &ArcShared<dyn MailboxSharedSetFactory>,
  ) -> Self {
    Self {
      core: DispatcherCore::new(settings, executor),
      shared_queue,
      team: Vec::new(),
      mailbox_shared_set_factory: mailbox_shared_set_factory.clone(),
    }
  }

  /// Returns a clone of the shared message queue used by team members.
  #[must_use]
  pub fn shared_queue(&self) -> SharedMessageQueue {
    self.shared_queue.clone()
  }

  /// Returns the number of currently registered team members.
  #[must_use]
  pub fn team_size(&self) -> usize {
    self.team.iter().filter(|weak| weak.upgrade().is_some()).count()
  }

  fn push_team_member(&mut self, actor: &ArcShared<ActorCell>) {
    let target_pid = actor.pid();
    if self.team.iter().any(|weak| weak.upgrade().is_some_and(|cell| cell.pid() == target_pid)) {
      return;
    }
    self.team.push(actor.downgrade());
  }

  fn remove_team_member(&mut self, actor: &ArcShared<ActorCell>) {
    let target_pid = actor.pid();
    self.team.retain(|weak| match weak.upgrade() {
      | Some(cell) => cell.pid() != target_pid,
      | None => false,
    });
  }

  fn collect_team_mailboxes(&mut self, primary: ArcShared<Mailbox>, primary_pid: Pid) -> Vec<ArcShared<Mailbox>> {
    let mut candidates: Vec<ArcShared<Mailbox>> = Vec::with_capacity(self.team.len() + 1);
    candidates.push(primary);

    self.team.retain(|weak| weak.upgrade().is_some());

    for weak in &self.team {
      let Some(cell) = weak.upgrade() else {
        continue;
      };
      if cell.pid() == primary_pid {
        continue;
      }
      let mbox = cell.mailbox();
      if !candidates.iter().any(|existing| ArcShared::ptr_eq(existing, &mbox)) {
        candidates.push(mbox);
      }
    }

    candidates
  }
}

impl MessageDispatcher for BalancingDispatcher {
  fn core(&self) -> &DispatcherCore {
    &self.core
  }

  fn core_mut(&mut self) -> &mut DispatcherCore {
    &mut self.core
  }

  fn try_create_shared_mailbox(&self) -> Option<ArcShared<Mailbox>> {
    // All team members must drain the same queue, so we construct a sharing
    // mailbox that delegates to `self.shared_queue` rather than letting
    // `ActorCell::create` build a fresh per-actor queue. The queue is stable
    // for the dispatcher's lifetime, so every call returns a mailbox that
    // wraps the same underlying `SharedMessageQueue`.
    let queue: Box<dyn MessageQueue> = Box::new(SharedMessageQueueBox(self.shared_queue.clone()));
    let shared_set = self.mailbox_shared_set_factory.create();
    Some(ArcShared::new(Mailbox::new_sharing_with_shared_set(MailboxPolicy::unbounded(None), queue, &shared_set)))
  }

  fn register_actor(&mut self, actor: &ArcShared<ActorCell>) -> Result<(), SpawnError> {
    self.core.mark_attach();
    self.push_team_member(actor);
    Ok(())
  }

  fn unregister_actor(&mut self, actor: &ArcShared<ActorCell>) {
    self.remove_team_member(actor);
    self.core.mark_detach();
  }

  fn dispatch(
    &mut self,
    receiver: &ArcShared<ActorCell>,
    envelope: Envelope,
  ) -> Result<Vec<ArcShared<Mailbox>>, SendError> {
    self.shared_queue.enqueue(envelope)?;
    let primary_mailbox = receiver.mailbox();
    let primary_pid = receiver.pid();
    Ok(self.collect_team_mailboxes(primary_mailbox, primary_pid))
  }

  fn system_dispatch(
    &mut self,
    receiver: &ArcShared<ActorCell>,
    message: SystemMessage,
  ) -> Result<Vec<ArcShared<Mailbox>>, SendError> {
    let mailbox = receiver.mailbox();
    mailbox.enqueue_system(message)?;
    Ok(alloc::vec![mailbox])
  }
}

/// Adapter that exposes [`SharedMessageQueue`] through the [`MessageQueue`] trait.
struct SharedMessageQueueBox(SharedMessageQueue);

impl MessageQueue for SharedMessageQueueBox {
  fn enqueue(&self, envelope: Envelope) -> Result<(), SendError> {
    self.0.enqueue(envelope)
  }

  fn dequeue(&self) -> Option<Envelope> {
    self.0.dequeue()
  }

  fn number_of_messages(&self) -> usize {
    self.0.number_of_messages()
  }

  fn has_messages(&self) -> bool {
    self.0.has_messages()
  }

  fn clean_up(&self) {
    self.0.clean_up();
  }
}
