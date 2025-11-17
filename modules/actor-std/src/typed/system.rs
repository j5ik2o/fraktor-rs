use fraktor_actor_core_rs::core::{
  actor_prim::Pid, event_stream::EventStreamSubscriber as CoreEventStreamSubscriber, logging::LogLevel,
  spawn::SpawnError, typed::TypedActorSystemGeneric as CoreTypedActorSystemGeneric,
};
use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

use crate::{
  dead_letter::DeadLetterEntry,
  error::SendError,
  event_stream::{
    EventStream, EventStreamEvent, EventStreamSubscriber, EventStreamSubscriberAdapter, EventStreamSubscription,
  },
  futures::ActorFuture,
  system::SystemState,
  typed::{TypedProps, actor_prim::TypedActorRef},
};

/// Typed actor system specialized for `StdToolbox`.
///
/// This is a newtype wrapper that provides std-specific convenience methods,
/// particularly for event stream operations with type conversions.
pub struct TypedActorSystem<M>
where
  M: Send + Sync + 'static, {
  inner: CoreTypedActorSystemGeneric<M, StdToolbox>,
}

impl<M> TypedActorSystem<M>
where
  M: Send + Sync + 'static,
{
  /// Creates an empty typed actor system (for testing).
  pub fn new_empty() -> Self {
    Self { inner: CoreTypedActorSystemGeneric::new_empty() }
  }

  /// Creates a new typed actor system with the given guardian props.
  ///
  /// # Errors
  ///
  /// Returns an error if the guardian actor cannot be spawned.
  pub fn new(guardian: &TypedProps<M>) -> Result<Self, SpawnError> {
    Ok(Self { inner: CoreTypedActorSystemGeneric::new(guardian.as_core())? })
  }

  /// Returns the typed user guardian reference.
  #[must_use]
  pub fn user_guardian_ref(&self) -> TypedActorRef<M> {
    TypedActorRef::from_core(self.inner.user_guardian_ref())
  }

  /// Returns the shared system state handle.
  #[must_use]
  pub fn state(&self) -> ArcShared<SystemState> {
    self.inner.state()
  }

  /// Allocates a new pid (testing helper).
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    self.inner.allocate_pid()
  }

  /// Returns the shared event stream handle.
  #[must_use]
  pub fn event_stream(&self) -> ArcShared<EventStream> {
    self.inner.event_stream()
  }

  /// Subscribes the provided observer to the event stream.
  ///
  /// This method provides std-specific type conversion from the local
  /// `EventStreamSubscriber` trait to the core trait.
  #[must_use]
  pub fn subscribe_event_stream(&self, subscriber: &ArcShared<dyn EventStreamSubscriber>) -> EventStreamSubscription {
    let adapter: ArcShared<dyn CoreEventStreamSubscriber<StdToolbox>> =
      ArcShared::new(EventStreamSubscriberAdapter::new(subscriber.clone()));
    self.inner.subscribe_event_stream(&adapter)
  }

  /// Returns a snapshot of recorded dead letters.
  #[must_use]
  pub fn dead_letters(&self) -> Vec<DeadLetterEntry> {
    self.inner.dead_letters()
  }

  /// Emits a log event with the specified severity.
  pub fn emit_log(&self, level: LogLevel, message: impl Into<String>, origin: Option<Pid>) {
    self.inner.emit_log(level, message, origin);
  }

  /// Publishes a raw event to the event stream.
  pub fn publish_event(&self, event: &EventStreamEvent) {
    self.inner.publish_event(event);
  }

  /// Returns a future that resolves once the actor system terminates.
  #[must_use]
  pub fn when_terminated(&self) -> ArcShared<ActorFuture<()>> {
    self.inner.when_terminated()
  }

  /// Sends a stop signal to the user guardian and initiates system shutdown.
  ///
  /// # Errors
  ///
  /// Returns an error if the terminate signal cannot be sent.
  pub fn terminate(&self) -> Result<(), SendError> {
    self.inner.terminate()
  }
}
