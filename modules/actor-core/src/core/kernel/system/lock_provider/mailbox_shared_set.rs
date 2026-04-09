//! Mailbox lock bundle materialized by an [`ActorLockProvider`](super::ActorLockProvider).

use fraktor_utils_core_rs::core::sync::WeakShared;

use crate::core::kernel::{
  actor::{ActorCell, messaging::message_invoker::MessageInvokerShared},
  dispatch::mailbox::MailboxInstrumentation,
  system::lock_provider::SharedLock,
};

/// Lock bundle used by mailbox hot-path state.
#[derive(Clone)]
pub struct MailboxSharedSet {
  user_queue_lock: MailboxLocked<()>,
  instrumentation: MailboxLocked<Option<MailboxInstrumentation>>,
  invoker:         MailboxLocked<Option<MessageInvokerShared>>,
  actor:           MailboxLocked<Option<WeakShared<ActorCell>>>,
}

impl MailboxSharedSet {
  pub(crate) fn builtin() -> Self {
    Self {
      user_queue_lock: MailboxLocked::builtin(()),
      instrumentation: MailboxLocked::builtin(None),
      invoker:         MailboxLocked::builtin(None),
      actor:           MailboxLocked::builtin(None),
    }
  }

  pub(crate) fn debug() -> Self {
    Self {
      user_queue_lock: MailboxLocked::debug((), "mailbox.user_queue_lock"),
      instrumentation: MailboxLocked::debug(None, "mailbox.instrumentation"),
      invoker:         MailboxLocked::debug(None, "mailbox.invoker"),
      actor:           MailboxLocked::debug(None, "mailbox.actor"),
    }
  }

  pub(crate) fn user_queue_lock(&self) -> MailboxLocked<()> {
    self.user_queue_lock.clone()
  }

  pub(crate) fn instrumentation(&self) -> MailboxLocked<Option<MailboxInstrumentation>> {
    self.instrumentation.clone()
  }

  pub(crate) fn invoker(&self) -> MailboxLocked<Option<MessageInvokerShared>> {
    self.invoker.clone()
  }

  pub(crate) fn actor(&self) -> MailboxLocked<Option<WeakShared<ActorCell>>> {
    self.actor.clone()
  }
}

pub(crate) type MailboxLocked<T> = SharedLock<T>;
