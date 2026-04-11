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
  event::stream::{EventStreamSubscriber, EventStreamSubscriberShared},
  system::lock_provider::{ActorLockProvider, MailboxSharedSet},
};
use fraktor_utils_adaptor_std_rs::std::sync::StdSyncMutex;
use fraktor_utils_core_rs::core::sync::{SharedLock, WeakShared};

/// Std lock provider backed by `std::sync::Mutex`.
#[derive(Default)]
pub struct StdActorLockProvider;

impl StdActorLockProvider {
  /// Creates the std provider.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl ActorLockProvider for StdActorLockProvider {
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared {
    MessageDispatcherShared::from_shared_lock(SharedLock::new_with_driver::<StdSyncMutex<Box<dyn MessageDispatcher>>>(
      dispatcher,
    ))
  }

  fn create_executor_shared(&self, executor: Box<dyn Executor>) -> ExecutorShared {
    ExecutorShared::from_parts(
      SharedLock::new_with_driver::<StdSyncMutex<Box<dyn Executor>>>(executor),
      SharedLock::new_with_driver::<StdSyncMutex<TrampolineState>>(TrampolineState::new()),
    )
  }

  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    ActorRefSenderShared::from_shared_lock(SharedLock::new_with_driver::<StdSyncMutex<Box<dyn ActorRefSender>>>(sender))
  }

  fn create_mailbox_shared_set(&self) -> MailboxSharedSet {
    MailboxSharedSet::new(
      SharedLock::new_with_driver::<StdSyncMutex<()>>(()),
      SharedLock::new_with_driver::<StdSyncMutex<Option<MailboxInstrumentation>>>(None),
      SharedLock::new_with_driver::<StdSyncMutex<Option<MessageInvokerShared>>>(None),
      SharedLock::new_with_driver::<StdSyncMutex<Option<WeakShared<ActorCell>>>>(None),
    )
  }

  fn create_event_stream_subscriber_shared(
    &self,
    subscriber: Box<dyn EventStreamSubscriber>,
  ) -> EventStreamSubscriberShared {
    SharedLock::new_with_driver::<StdSyncMutex<Box<dyn EventStreamSubscriber>>>(subscriber)
  }
}
