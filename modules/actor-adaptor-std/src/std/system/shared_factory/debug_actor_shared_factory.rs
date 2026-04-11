use alloc::{boxed::Box, collections::VecDeque};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorCell, ActorCellStateShared, ActorRuntimeLockFactory, ReceiveTimeoutStateShared,
    actor_ref::{ActorRefSender, ActorRefSenderShared},
    messaging::message_invoker::{MessageInvoker, MessageInvokerShared},
  },
  dispatch::{
    dispatcher::{
      Executor, ExecutorShared, MessageDispatcher, MessageDispatcherShared, SharedMessageQueue, TrampolineState,
    },
    mailbox::MailboxInstrumentation,
  },
  event::stream::{EventStream, EventStreamShared, EventStreamSubscriber, EventStreamSubscriberShared},
  system::shared_factory::{ActorSharedFactory, MailboxSharedSet},
};
use fraktor_utils_adaptor_std_rs::std::sync::{DebugSpinSyncMutex, DebugSpinSyncRwLock};
use fraktor_utils_core_rs::core::sync::{SharedLock, SharedRwLock, WeakShared};

/// Debug shared factory that panics on same-thread hot-path re-entry.
#[derive(Default)]
pub struct DebugActorSharedFactory;

impl DebugActorSharedFactory {
  /// Creates the debug provider.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }

  fn create_lock<T>(value: T) -> SharedLock<T>
  where
    T: Send + 'static, {
    SharedLock::new_with_driver::<DebugSpinSyncMutex<_>>(value)
  }

  fn create_rw_lock<T>(value: T) -> SharedRwLock<T>
  where
    T: Send + Sync + 'static, {
    SharedRwLock::new_with_driver::<DebugSpinSyncRwLock<_>>(value)
  }
}

impl ActorRuntimeLockFactory for DebugActorSharedFactory {
  fn create_lock<T>(&self, value: T) -> SharedLock<T>
  where
    T: Send + 'static, {
    Self::create_lock(value)
  }
}

impl ActorSharedFactory for DebugActorSharedFactory {
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared {
    MessageDispatcherShared::from_shared_lock(Self::create_lock(dispatcher))
  }

  fn create_executor_shared(&self, executor: Box<dyn Executor>) -> ExecutorShared {
    ExecutorShared::from_parts(Self::create_lock(executor), Self::create_lock(TrampolineState::new()))
  }

  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    ActorRefSenderShared::from_shared_lock(Self::create_lock(sender))
  }

  fn create_actor_shared_lock(&self, actor: Box<dyn Actor + Send + Sync>) -> SharedLock<Box<dyn Actor + Send + Sync>> {
    Self::create_lock(actor)
  }

  fn create_actor_cell_state_shared(&self) -> ActorCellStateShared {
    ActorCellStateShared::new_with_lock_factory(self)
  }

  fn create_receive_timeout_state_shared(&self) -> ReceiveTimeoutStateShared {
    ReceiveTimeoutStateShared::new_with_lock_factory(self)
  }

  fn create_message_invoker_shared(&self, invoker: Box<dyn MessageInvoker>) -> MessageInvokerShared {
    MessageInvokerShared::from_shared_lock(Self::create_rw_lock(invoker))
  }

  fn create_shared_message_queue(&self) -> SharedMessageQueue {
    SharedMessageQueue::from_shared_lock(Self::create_lock(VecDeque::new()))
  }

  fn create_event_stream_shared(&self, stream: EventStream) -> EventStreamShared {
    EventStreamShared::from_shared_lock(Self::create_rw_lock(stream))
  }

  fn create_event_stream_subscriber_shared(
    &self,
    subscriber: Box<dyn EventStreamSubscriber>,
  ) -> EventStreamSubscriberShared {
    Self::create_lock(subscriber)
  }

  fn create_mailbox_shared_set(&self) -> MailboxSharedSet {
    MailboxSharedSet::new(
      Self::create_lock(()),
      Self::create_lock(Option::<MailboxInstrumentation>::None),
      Self::create_lock(Option::<MessageInvokerShared>::None),
      Self::create_lock(Option::<WeakShared<ActorCell>>::None),
    )
  }
}
