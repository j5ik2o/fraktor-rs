use alloc::boxed::Box;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    ActorCell,
    actor_ref::{ActorRefSender, ActorRefSenderShared},
    messaging::message_invoker::MessageInvokerShared,
  },
  dispatch::{
    dispatcher::{Executor, ExecutorShared, MessageDispatcher, MessageDispatcherShared, TrampolineState},
    mailbox::MailboxInstrumentation,
  },
  system::lock_provider::{ActorLockProvider, MailboxSharedSet},
};
use fraktor_utils_adaptor_std_rs::std::sync::DebugSpinSyncMutex;
use fraktor_utils_core_rs::core::sync::{SharedLock, WeakShared};

/// Debug lock provider that panics on same-thread hot-path re-entry.
#[derive(Default)]
pub struct DebugActorLockProvider;

impl DebugActorLockProvider {
  /// Creates the debug provider.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl ActorLockProvider for DebugActorLockProvider {
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared {
    MessageDispatcherShared::from_shared_lock(SharedLock::new_with_driver::<
      DebugSpinSyncMutex<Box<dyn MessageDispatcher>>,
    >(dispatcher))
  }

  fn create_executor_shared(&self, executor: Box<dyn Executor>) -> ExecutorShared {
    ExecutorShared::from_parts(
      SharedLock::new_with_driver::<DebugSpinSyncMutex<Box<dyn Executor>>>(executor),
      SharedLock::new_with_driver::<DebugSpinSyncMutex<TrampolineState>>(TrampolineState::new()),
    )
  }

  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    ActorRefSenderShared::from_shared_lock(SharedLock::new_with_driver::<DebugSpinSyncMutex<Box<dyn ActorRefSender>>>(
      sender,
    ))
  }

  fn create_mailbox_shared_set(&self) -> MailboxSharedSet {
    MailboxSharedSet::new(
      SharedLock::new_with_driver::<DebugSpinSyncMutex<()>>(()),
      SharedLock::new_with_driver::<DebugSpinSyncMutex<Option<MailboxInstrumentation>>>(None),
      SharedLock::new_with_driver::<DebugSpinSyncMutex<Option<MessageInvokerShared>>>(None),
      SharedLock::new_with_driver::<DebugSpinSyncMutex<Option<WeakShared<ActorCell>>>>(None),
    )
  }
}
