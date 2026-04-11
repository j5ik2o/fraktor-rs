//! Actor-system scoped hot-path lock provider.

use alloc::boxed::Box;

use crate::core::kernel::{
  actor::actor_ref::{ActorRefSender, ActorRefSenderShared},
  dispatch::dispatcher::{Executor, ExecutorShared, MessageDispatcher, MessageDispatcherShared},
  event::stream::{EventStreamSubscriber, EventStreamSubscriberShared},
  system::lock_provider::MailboxSharedSet,
};

/// Factory contract for actor-system hot-path shared wrappers.
///
/// `ActorLockProvider` is the actor system's runtime override seam: when an
/// implementation is installed via
/// [`ActorSystemConfig::with_lock_provider`](crate::core::kernel::actor::setup::actor_system_config::ActorSystemConfig::with_lock_provider)
/// each `*Shared` wrapper that the actor system materializes goes through
/// these factory methods instead of the workspace's compile-time selected
/// default lock driver. This is the canonical hook used by:
///
/// - `DebugActorLockProvider` (test builds): swap in `DebugSpinSyncMutex` to detect re-entrant lock
///   acquisition,
/// - `StdActorLockProvider` (std builds): swap in `std::sync::Mutex` to play nicely with tokio
///   worker threads,
/// - `ParkingLotActorLockProvider` (production): swap in `parking_lot::Mutex` for the same reason
///   but with better contention behaviour.
///
/// New code paths should *not* propagate the provider into deeper
/// constructors — the override is intentionally limited to the small set of
/// hot-path factories listed below so that the bulk of the workspace can
/// keep using `SharedLock::new` directly.
pub trait ActorLockProvider: Send + Sync {
  /// Materializes a dispatcher shared wrapper.
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared;

  /// Materializes an executor shared wrapper.
  fn create_executor_shared(&self, executor: Box<dyn Executor>) -> ExecutorShared;

  /// Materializes an actor-ref sender shared wrapper.
  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared;

  /// Materializes a mailbox lock bundle.
  fn create_mailbox_shared_set(&self) -> MailboxSharedSet;

  /// Materializes an event-stream subscriber shared wrapper.
  ///
  /// This is the route that
  /// [`subscriber_handle_with_lock_provider`](crate::core::kernel::event::stream::subscriber_handle_with_lock_provider)
  /// uses to swap the subscriber's lock driver, which is required for the
  /// `DebugActorLockProvider` re-entrant subscriber detection test in
  /// `actor-adaptor-std` to actually exercise its instrumentation.
  fn create_event_stream_subscriber_shared(
    &self,
    subscriber: Box<dyn EventStreamSubscriber>,
  ) -> EventStreamSubscriberShared;
}
