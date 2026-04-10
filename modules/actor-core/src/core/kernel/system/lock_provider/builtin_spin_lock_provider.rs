//! Built-in actor lock provider backed by the canonical spin mutex.

use alloc::boxed::Box;

use crate::core::kernel::{
  actor::actor_ref::{ActorRefSender, ActorRefSenderShared},
  dispatch::dispatcher::{Executor, ExecutorShared, MessageDispatcher, MessageDispatcherShared, TrampolineState},
  system::lock_provider::{ActorLockProvider, MailboxSharedSet, SharedLock},
};

/// Default lock provider used by `ActorSystemConfig::default()`.
#[derive(Default)]
pub struct BuiltinSpinLockProvider;

impl BuiltinSpinLockProvider {
  /// Creates the built-in provider.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl ActorLockProvider for BuiltinSpinLockProvider {
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared {
    MessageDispatcherShared::from_shared_lock(SharedLock::builtin(dispatcher))
  }

  fn create_executor_shared(&self, executor: Box<dyn Executor>) -> ExecutorShared {
    ExecutorShared::from_parts(SharedLock::builtin(executor), SharedLock::builtin(TrampolineState::new()))
  }

  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    ActorRefSenderShared::from_shared_lock(SharedLock::builtin(sender))
  }

  fn create_mailbox_shared_set(&self) -> MailboxSharedSet {
    MailboxSharedSet::builtin()
  }
}
