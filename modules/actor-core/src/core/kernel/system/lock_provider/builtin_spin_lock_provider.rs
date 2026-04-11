//! Built-in actor lock provider backed by the workspace's compile-time
//! selected default lock driver.
//!
//! This provider exists primarily as a thin trait-object adapter so that
//! code paths still expecting an `ActorLockProvider` (e.g. legacy
//! `*_with_provider` factories used by tests) can keep working while the
//! main path constructs `*Shared` wrappers directly via `SharedLock::new`.
//!
//! New code should not propagate `BuiltinSpinLockProvider` through
//! constructors — use the provider-less factories
//! (`MessageDispatcherShared::new_with_builtin_lock`, etc.) instead and
//! reach for an `ActorLockProvider` only at the actor system boundary
//! (`ActorSystemConfig::with_lock_provider`) when a custom mutex backing is
//! required at runtime (DebugSpinSyncMutex, parking_lot, …).

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::SharedLock;

use crate::core::kernel::{
  actor::actor_ref::{ActorRefSender, ActorRefSenderShared},
  dispatch::dispatcher::{Executor, ExecutorShared, MessageDispatcher, MessageDispatcherShared, TrampolineState},
  event::stream::{EventStreamSubscriber, EventStreamSubscriberShared},
  system::lock_provider::{ActorLockProvider, MailboxSharedSet},
};

/// Trait-object adapter that materializes `*Shared` wrappers using the
/// workspace's compile-time selected default lock driver.
///
/// `ActorSystemConfig::default()` no longer instantiates this provider;
/// shared wrappers are built directly via the `*Shared::new_with_builtin_lock`
/// helpers. The provider remains exported so override-style call sites that
/// genuinely need to dispatch through `dyn ActorLockProvider` (for symmetry
/// with `StdActorLockProvider` / `DebugActorLockProvider`) can still pick
/// the default behaviour explicitly.
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
    MessageDispatcherShared::from_shared_lock(SharedLock::new(dispatcher))
  }

  fn create_executor_shared(&self, executor: Box<dyn Executor>) -> ExecutorShared {
    ExecutorShared::from_parts(SharedLock::new(executor), SharedLock::new(TrampolineState::new()))
  }

  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    ActorRefSenderShared::from_shared_lock(SharedLock::new(sender))
  }

  fn create_mailbox_shared_set(&self) -> MailboxSharedSet {
    MailboxSharedSet::with_builtin_lock()
  }

  fn create_event_stream_subscriber_shared(
    &self,
    subscriber: Box<dyn EventStreamSubscriber>,
  ) -> EventStreamSubscriberShared {
    EventStreamSubscriberShared::new(subscriber)
  }
}
