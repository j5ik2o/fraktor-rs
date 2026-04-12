use alloc::{boxed::Box, collections::VecDeque};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorCell, ActorCellState, ActorCellStateShared, ActorCellStateSharedFactory, ActorLockFactory,
    ActorSharedLockFactory, ReceiveTimeoutState, ReceiveTimeoutStateShared, ReceiveTimeoutStateSharedFactory,
    actor_ref::{ActorRefSender, ActorRefSenderShared, ActorRefSenderSharedFactory},
    actor_ref_provider::{
      ActorRefProvider, ActorRefProviderHandle, ActorRefProviderHandleShared, ActorRefProviderHandleSharedFactory,
    },
    messaging::message_invoker::{MessageInvoker, MessageInvokerShared, MessageInvokerSharedFactory},
    scheduler::tick_driver::{TickDriverControl, TickDriverControlShared, TickDriverControlSharedFactory},
  },
  dispatch::{
    dispatcher::{
      Executor, ExecutorShared, ExecutorSharedFactory, MessageDispatcher, MessageDispatcherShared,
      MessageDispatcherSharedFactory, SharedMessageQueue, SharedMessageQueueFactory, TrampolineState,
    },
    mailbox::{
      BoundedPriorityMessageQueueState, BoundedPriorityMessageQueueStateShared,
      BoundedPriorityMessageQueueStateSharedFactory, MailboxInstrumentation,
    },
  },
  event::stream::{
    EventStream, EventStreamShared, EventStreamSharedFactory, EventStreamSubscriber, EventStreamSubscriberShared,
    EventStreamSubscriberSharedFactory,
  },
  system::{
    remote::{RemoteWatchHook, RemoteWatchHookHandle, RemoteWatchHookHandleShared, RemoteWatchHookHandleSharedFactory},
    shared_factory::{MailboxSharedSet, MailboxSharedSetFactory},
  },
  util::futures::{ActorFuture, ActorFutureShared, ActorFutureSharedFactory},
};
use fraktor_utils_adaptor_std_rs::std::sync::{StdSyncMutex, StdSyncRwLock};
use fraktor_utils_core_rs::core::sync::{SharedLock, SharedRwLock, WeakShared};

/// Std shared factory backed by `std::sync::Mutex`.
#[derive(Default)]
pub struct StdActorSharedFactory;

impl StdActorSharedFactory {
  /// Creates the std provider.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }

  fn create_lock<T>(value: T) -> SharedLock<T>
  where
    T: Send + 'static, {
    SharedLock::new_with_driver::<StdSyncMutex<_>>(value)
  }

  fn create_rw_lock<T>(value: T) -> SharedRwLock<T>
  where
    T: Send + Sync + 'static, {
    SharedRwLock::new_with_driver::<StdSyncRwLock<_>>(value)
  }
}

impl ActorLockFactory for StdActorSharedFactory {
  fn create_lock<T>(&self, value: T) -> SharedLock<T>
  where
    T: Send + 'static, {
    Self::create_lock(value)
  }
}

impl MessageDispatcherSharedFactory for StdActorSharedFactory {
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared {
    MessageDispatcherShared::from_shared_lock(Self::create_lock(dispatcher))
  }
}

impl ExecutorSharedFactory for StdActorSharedFactory {
  fn create_executor_shared(&self, executor: Box<dyn Executor>, trampoline: TrampolineState) -> ExecutorShared {
    ExecutorShared::from_shared_lock(Self::create_lock(executor), Self::create_lock(trampoline))
  }
}

impl ActorRefSenderSharedFactory for StdActorSharedFactory {
  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    ActorRefSenderShared::from_shared_lock(Self::create_lock(sender))
  }
}

impl ActorSharedLockFactory for StdActorSharedFactory {
  fn create(&self, actor: Box<dyn Actor + Send>) -> SharedLock<Box<dyn Actor + Send>> {
    Self::create_lock(actor)
  }
}

impl ActorCellStateSharedFactory for StdActorSharedFactory {
  fn create_actor_cell_state_shared(&self, state: ActorCellState) -> ActorCellStateShared {
    ActorCellStateShared::from_shared_lock(Self::create_lock(state))
  }
}

impl ReceiveTimeoutStateSharedFactory for StdActorSharedFactory {
  fn create_receive_timeout_state_shared(&self, state: Option<ReceiveTimeoutState>) -> ReceiveTimeoutStateShared {
    ReceiveTimeoutStateShared::from_shared_lock(Self::create_lock(state))
  }
}

impl MessageInvokerSharedFactory for StdActorSharedFactory {
  fn create(&self, invoker: Box<dyn MessageInvoker>) -> MessageInvokerShared {
    MessageInvokerShared::from_shared_lock(Self::create_rw_lock(invoker))
  }
}

impl SharedMessageQueueFactory for StdActorSharedFactory {
  fn create(&self) -> SharedMessageQueue {
    SharedMessageQueue::from_shared_lock(Self::create_lock(VecDeque::new()))
  }
}

impl BoundedPriorityMessageQueueStateSharedFactory for StdActorSharedFactory {
  fn create_bounded_priority_message_queue_state_shared(
    &self,
    state: BoundedPriorityMessageQueueState,
  ) -> BoundedPriorityMessageQueueStateShared {
    BoundedPriorityMessageQueueStateShared::from_shared_lock(Self::create_lock(state))
  }
}

impl EventStreamSharedFactory for StdActorSharedFactory {
  fn create(&self, stream: EventStream) -> EventStreamShared {
    EventStreamShared::from_shared_lock(Self::create_rw_lock(stream))
  }
}

impl EventStreamSubscriberSharedFactory for StdActorSharedFactory {
  fn create(&self, subscriber: Box<dyn EventStreamSubscriber>) -> EventStreamSubscriberShared {
    EventStreamSubscriberShared::from_shared_lock(Self::create_lock(subscriber))
  }
}

impl MailboxSharedSetFactory for StdActorSharedFactory {
  fn create(&self) -> MailboxSharedSet {
    MailboxSharedSet::new(
      Self::create_lock(()),
      Self::create_lock(Option::<MailboxInstrumentation>::None),
      Self::create_lock(Option::<MessageInvokerShared>::None),
      Self::create_lock(Option::<WeakShared<ActorCell>>::None),
    )
  }
}

impl<T> ActorFutureSharedFactory<T> for StdActorSharedFactory
where
  T: Send + 'static,
{
  fn create_actor_future_shared(&self, future: ActorFuture<T>) -> ActorFutureShared<T> {
    ActorFutureShared::from_shared_lock(Self::create_lock(future))
  }
}

impl TickDriverControlSharedFactory for StdActorSharedFactory {
  fn create_tick_driver_control_shared(&self, control: Box<dyn TickDriverControl>) -> TickDriverControlShared {
    TickDriverControlShared::from_shared(Self::create_lock(control))
  }
}

impl<P> ActorRefProviderHandleSharedFactory<P> for StdActorSharedFactory
where
  P: ActorRefProvider + 'static,
{
  fn create_actor_ref_provider_handle_shared(&self, provider: P) -> ActorRefProviderHandleShared<P> {
    let schemes = provider.supported_schemes();
    ActorRefProviderHandleShared::from_shared(Self::create_lock(ActorRefProviderHandle::new(provider, schemes)))
  }
}

impl<P> RemoteWatchHookHandleSharedFactory<P> for StdActorSharedFactory
where
  P: ActorRefProvider + RemoteWatchHook + 'static,
{
  fn create_remote_watch_hook_handle_shared(&self, provider: P) -> RemoteWatchHookHandleShared<P> {
    let schemes = provider.supported_schemes();
    RemoteWatchHookHandleShared::from_shared(Self::create_lock(RemoteWatchHookHandle::new(provider, schemes)))
  }
}
