use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::SharedLock;

use super::ActorSharedFactory;
use crate::core::kernel::{
  actor::{
    Actor, ActorCellStateShared, ReceiveTimeoutStateShared,
    actor_ref::{ActorRefSender, ActorRefSenderShared},
    messaging::message_invoker::{MessageInvoker, MessageInvokerShared},
  },
  dispatch::dispatcher::{Executor, ExecutorShared, MessageDispatcher, MessageDispatcherShared, SharedMessageQueue},
  event::stream::{EventStream, EventStreamShared, EventStreamSubscriber, EventStreamSubscriberShared},
  system::shared_factory::{BuiltinSpinSharedFactory, MailboxSharedSet},
};

struct ContractSmokeProvider {
  inner: BuiltinSpinSharedFactory,
}

impl ContractSmokeProvider {
  const fn new() -> Self {
    Self { inner: BuiltinSpinSharedFactory::new() }
  }
}

impl ActorSharedFactory for ContractSmokeProvider {
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared {
    self.inner.create_message_dispatcher_shared(dispatcher)
  }

  fn create_executor_shared(&self, executor: Box<dyn Executor>) -> ExecutorShared {
    self.inner.create_executor_shared(executor)
  }

  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    self.inner.create_actor_ref_sender_shared(sender)
  }

  fn create_actor_shared_lock(&self, actor: Box<dyn Actor + Send + Sync>) -> SharedLock<Box<dyn Actor + Send + Sync>> {
    self.inner.create_actor_shared_lock(actor)
  }

  fn create_actor_cell_state_shared(&self) -> ActorCellStateShared {
    self.inner.create_actor_cell_state_shared()
  }

  fn create_receive_timeout_state_shared(&self) -> ReceiveTimeoutStateShared {
    self.inner.create_receive_timeout_state_shared()
  }

  fn create_message_invoker_shared(&self, invoker: Box<dyn MessageInvoker>) -> MessageInvokerShared {
    self.inner.create_message_invoker_shared(invoker)
  }

  fn create_shared_message_queue(&self) -> SharedMessageQueue {
    self.inner.create_shared_message_queue()
  }

  fn create_event_stream_shared(&self, stream: EventStream) -> EventStreamShared {
    self.inner.create_event_stream_shared(stream)
  }

  fn create_event_stream_subscriber_shared(
    &self,
    subscriber: Box<dyn EventStreamSubscriber>,
  ) -> EventStreamSubscriberShared {
    self.inner.create_event_stream_subscriber_shared(subscriber)
  }

  fn create_mailbox_shared_set(&self) -> MailboxSharedSet {
    self.inner.create_mailbox_shared_set()
  }
}

#[test]
fn actor_shared_factory_contract_is_implementable_without_runtime_state_types() {
  let _provider: Box<dyn ActorSharedFactory> = Box::new(ContractSmokeProvider::new());
}
