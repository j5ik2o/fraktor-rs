//! Actor runtime lock provider contract.

use alloc::boxed::Box;

use super::{DispatcherLockCell, ExecutorLockCell, MailboxLockSet, SenderLockCell};
use crate::core::kernel::{
  actor::actor_ref::ActorRefSender,
  dispatch::dispatcher::{Executor, MessageDispatcher},
};

/// Provider that materializes actor-runtime hot-path lock surfaces.
pub trait ActorRuntimeLockProvider: Send + Sync {
  /// Wraps a dispatcher in an opaque lock cell.
  fn new_dispatcher_cell(&self, dispatcher: Box<dyn MessageDispatcher>) -> DispatcherLockCell;

  /// Wraps an executor in an opaque lock cell.
  fn new_executor_cell(&self, executor: Box<dyn Executor>) -> ExecutorLockCell;

  /// Wraps an actor-ref sender in an opaque lock cell.
  fn new_sender_cell(&self, sender: Box<dyn ActorRefSender>) -> SenderLockCell;

  /// Creates a fresh mailbox lock bundle for a single mailbox instance.
  fn new_mailbox_lock_set(&self) -> MailboxLockSet;
}
