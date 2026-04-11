//! Actor-system scoped hot-path shared factory.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::SharedLock;

use crate::core::kernel::{
  actor::{
    Actor, ActorCellStateShared, ReceiveTimeoutStateShared,
    actor_ref::{ActorRefSender, ActorRefSenderShared},
    messaging::message_invoker::{MessageInvoker, MessageInvokerShared},
  },
  dispatch::dispatcher::{Executor, ExecutorShared, MessageDispatcher, MessageDispatcherShared, SharedMessageQueue},
  event::stream::{EventStream, EventStreamShared, EventStreamSubscriber, EventStreamSubscriberShared},
  system::shared_factory::MailboxSharedSet,
};

/// Factory contract for actor-system hot-path shared wrappers.
pub trait ActorSharedFactory: Send + Sync {
  /// Materializes a dispatcher shared wrapper.
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared;

  /// Materializes an executor shared wrapper.
  fn create_executor_shared(&self, executor: Box<dyn Executor>) -> ExecutorShared;

  /// Materializes an actor-ref sender shared wrapper.
  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared;

  /// Materializes an actor instance lock for actor-cell owned runtime state.
  fn create_actor_shared_lock(&self, actor: Box<dyn Actor + Send + Sync>) -> SharedLock<Box<dyn Actor + Send + Sync>>;

  /// Materializes the shared actor-cell runtime state bundle.
  fn create_actor_cell_state_shared(&self) -> ActorCellStateShared;

  /// Materializes the shared receive-timeout slot used by actor contexts.
  fn create_receive_timeout_state_shared(&self) -> ReceiveTimeoutStateShared;

  /// Materializes a message invoker shared wrapper.
  fn create_message_invoker_shared(&self, invoker: Box<dyn MessageInvoker>) -> MessageInvokerShared;

  /// Materializes the shared queue used by balancing dispatchers.
  fn create_shared_message_queue(&self) -> SharedMessageQueue;

  /// Materializes an event-stream shared wrapper.
  fn create_event_stream_shared(&self, stream: EventStream) -> EventStreamShared;

  /// Materializes an event-stream subscriber shared wrapper.
  fn create_event_stream_subscriber_shared(
    &self,
    subscriber: Box<dyn EventStreamSubscriber>,
  ) -> EventStreamSubscriberShared;

  /// Materializes a mailbox lock bundle.
  fn create_mailbox_shared_set(&self) -> MailboxSharedSet;
}
