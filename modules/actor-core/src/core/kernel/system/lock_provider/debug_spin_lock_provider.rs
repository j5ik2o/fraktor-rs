//! Debug actor lock provider that panics on lock contention.

use alloc::boxed::Box;

use crate::core::kernel::{
  actor::actor_ref::{ActorRefSender, ActorRefSenderShared},
  dispatch::dispatcher::{Executor, ExecutorShared, MessageDispatcher, MessageDispatcherShared, TrampolineState},
  system::lock_provider::{ActorLockProvider, MailboxSharedSet, SharedLock},
};

/// Debug lock provider that fails fast on re-entrant or contended hot-path locks.
#[derive(Default)]
pub struct DebugSpinLockProvider;

impl DebugSpinLockProvider {
  /// Creates the debug provider.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl ActorLockProvider for DebugSpinLockProvider {
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared {
    MessageDispatcherShared::from_shared_lock(SharedLock::debug(dispatcher, "message_dispatcher_shared.inner"))
  }

  fn create_executor_shared(&self, executor: Box<dyn Executor>) -> ExecutorShared {
    ExecutorShared::from_parts(
      SharedLock::debug(executor, "executor_shared.inner"),
      SharedLock::debug(TrampolineState::new(), "executor_shared.trampoline"),
    )
  }

  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    ActorRefSenderShared::from_shared_lock(SharedLock::debug(sender, "actor_ref_sender_shared.inner"))
  }

  fn create_mailbox_shared_set(&self) -> MailboxSharedSet {
    MailboxSharedSet::debug()
  }
}
