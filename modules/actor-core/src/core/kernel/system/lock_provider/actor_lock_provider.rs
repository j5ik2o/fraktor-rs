//! Actor-system scoped hot-path lock provider.

use alloc::boxed::Box;

use crate::core::kernel::{
  actor::actor_ref::{ActorRefSender, ActorRefSenderShared},
  dispatch::dispatcher::{Executor, ExecutorShared, MessageDispatcher, MessageDispatcherShared},
  system::lock_provider::MailboxSharedSet,
};

/// Factory contract for actor-system hot-path shared wrappers.
pub trait ActorLockProvider: Send + Sync {
  /// Materializes a dispatcher shared wrapper.
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared;

  /// Materializes an executor shared wrapper.
  fn create_executor_shared(&self, executor: Box<dyn Executor>) -> ExecutorShared;

  /// Materializes an actor-ref sender shared wrapper.
  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared;

  /// Materializes a mailbox lock bundle.
  fn create_mailbox_shared_set(&self) -> MailboxSharedSet;
}
