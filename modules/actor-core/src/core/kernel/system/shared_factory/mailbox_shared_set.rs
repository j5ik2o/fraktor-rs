//! Mailbox lock bundle materialized by a
//! [`MailboxSharedSetFactory`](super::MailboxSharedSetFactory).

use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex, WeakShared};

use crate::core::kernel::{
  actor::{ActorCell, messaging::message_invoker::MessageInvokerShared},
  dispatch::mailbox::MailboxInstrumentation,
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
  /// Creates a mailbox lock bundle from already materialized shared locks.
  #[must_use]
  pub const fn new(
    user_queue_lock: MailboxLocked<()>,
    instrumentation: MailboxLocked<Option<MailboxInstrumentation>>,
    invoker: MailboxLocked<Option<MessageInvokerShared>>,
    actor: MailboxLocked<Option<WeakShared<ActorCell>>>,
  ) -> Self {
    Self { user_queue_lock, instrumentation, invoker, actor }
  }

  pub(crate) fn builtin() -> Self {
    Self::new(
      MailboxLocked::new_with_driver::<SpinSyncMutex<()>>(()),
      MailboxLocked::new_with_driver::<SpinSyncMutex<Option<MailboxInstrumentation>>>(None),
      MailboxLocked::new_with_driver::<SpinSyncMutex<Option<MessageInvokerShared>>>(None),
      MailboxLocked::new_with_driver::<SpinSyncMutex<Option<WeakShared<ActorCell>>>>(None),
    )
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
