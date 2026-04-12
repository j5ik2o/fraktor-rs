//! Built-in actor shared factory backed by the canonical spin mutex.

use alloc::{boxed::Box, collections::VecDeque};

use fraktor_utils_core_rs::core::sync::{SharedLock, SharedRwLock, SpinSyncMutex, SpinSyncRwLock, WeakShared};

use crate::core::kernel::{
  actor::{
    Actor, ActorCell, ActorCellStateShared, ActorCellStateSharedFactory, ActorLockFactory, ActorSharedLockFactory,
    ReceiveTimeoutStateShared, ReceiveTimeoutStateSharedFactory,
    actor_ref::{ActorRefSender, ActorRefSenderShared, ActorRefSenderSharedFactory},
    actor_ref_provider::{
      ActorRefProvider, ActorRefProviderHandle, ActorRefProviderHandleShared, ActorRefProviderHandleSharedFactory,
    },
    messaging::message_invoker::{MessageInvoker, MessageInvokerShared, MessageInvokerSharedFactory},
  },
  dispatch::{
    dispatcher::{
      Executor, ExecutorShared, ExecutorSharedFactory, MessageDispatcher, MessageDispatcherShared,
      MessageDispatcherSharedFactory, SharedMessageQueue, SharedMessageQueueFactory, TrampolineState,
    },
    mailbox::MailboxInstrumentation,
  },
  event::stream::{
    EventStream, EventStreamShared, EventStreamSharedFactory, EventStreamSubscriber, EventStreamSubscriberShared,
    EventStreamSubscriberSharedFactory,
  },
  system::shared_factory::{MailboxSharedSet, MailboxSharedSetFactory},
  util::futures::{ActorFuture, ActorFutureShared, ActorFutureSharedFactory},
};

/// Default shared factory used by `ActorSystemConfig::default()`.
#[derive(Default)]
pub struct BuiltinSpinSharedFactory;

impl BuiltinSpinSharedFactory {
  /// Creates the built-in provider.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }

  fn create_lock<T>(value: T) -> SharedLock<T>
  where
    T: Send + 'static, {
    SharedLock::new_with_driver::<SpinSyncMutex<_>>(value)
  }

  fn create_rw_lock<T>(value: T) -> SharedRwLock<T>
  where
    T: Send + Sync + 'static, {
    SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(value)
  }
}

impl ActorLockFactory for BuiltinSpinSharedFactory {
  fn create_lock<T>(&self, value: T) -> SharedLock<T>
  where
    T: Send + 'static, {
    Self::create_lock(value)
  }
}

impl MessageDispatcherSharedFactory for BuiltinSpinSharedFactory {
  fn create(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared {
    MessageDispatcherShared::from_shared_lock(Self::create_lock(dispatcher))
  }
}

impl ExecutorSharedFactory for BuiltinSpinSharedFactory {
  fn create(&self, executor: Box<dyn Executor>) -> ExecutorShared {
    ExecutorShared::from_parts(Self::create_lock(executor), Self::create_lock(TrampolineState::new()))
  }
}

impl ActorRefSenderSharedFactory for BuiltinSpinSharedFactory {
  fn create(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    ActorRefSenderShared::from_shared_lock(Self::create_lock(sender))
  }
}

impl ActorSharedLockFactory for BuiltinSpinSharedFactory {
  fn create(&self, actor: Box<dyn Actor + Send>) -> SharedLock<Box<dyn Actor + Send>> {
    Self::create_lock(actor)
  }
}

impl ActorCellStateSharedFactory for BuiltinSpinSharedFactory {
  fn create(&self) -> ActorCellStateShared {
    ActorCellStateShared::new_with_lock_factory(self)
  }
}

impl ReceiveTimeoutStateSharedFactory for BuiltinSpinSharedFactory {
  fn create(&self) -> ReceiveTimeoutStateShared {
    ReceiveTimeoutStateShared::new_with_lock_factory(self)
  }
}

impl MessageInvokerSharedFactory for BuiltinSpinSharedFactory {
  fn create(&self, invoker: Box<dyn MessageInvoker>) -> MessageInvokerShared {
    MessageInvokerShared::from_shared_lock(Self::create_rw_lock(invoker))
  }
}

impl SharedMessageQueueFactory for BuiltinSpinSharedFactory {
  fn create(&self) -> SharedMessageQueue {
    SharedMessageQueue::from_shared_lock(Self::create_lock(VecDeque::new()))
  }
}

impl EventStreamSharedFactory for BuiltinSpinSharedFactory {
  fn create(&self, stream: EventStream) -> EventStreamShared {
    EventStreamShared::from_shared_lock(Self::create_rw_lock(stream))
  }
}

impl EventStreamSubscriberSharedFactory for BuiltinSpinSharedFactory {
  fn create(&self, subscriber: Box<dyn EventStreamSubscriber>) -> EventStreamSubscriberShared {
    Self::create_lock(subscriber)
  }
}

impl MailboxSharedSetFactory for BuiltinSpinSharedFactory {
  fn create(&self) -> MailboxSharedSet {
    MailboxSharedSet::new(
      Self::create_lock(()),
      Self::create_lock(Option::<MailboxInstrumentation>::None),
      Self::create_lock(Option::<MessageInvokerShared>::None),
      Self::create_lock(Option::<WeakShared<ActorCell>>::None),
    )
  }
}

impl<T> ActorFutureSharedFactory<T> for BuiltinSpinSharedFactory
where
  T: Send + 'static,
{
  fn create_actor_future_shared(&self, future: ActorFuture<T>) -> ActorFutureShared<T> {
    ActorFutureShared::from_shared_lock(Self::create_lock(future))
  }
}

impl<P> ActorRefProviderHandleSharedFactory<P> for BuiltinSpinSharedFactory
where
  P: ActorRefProvider + 'static,
{
  fn create_actor_ref_provider_handle_shared(&self, provider: P) -> ActorRefProviderHandleShared<P> {
    let schemes = provider.supported_schemes();
    ActorRefProviderHandleShared::from_shared(Self::create_lock(ActorRefProviderHandle::new(provider, schemes)))
  }
}
