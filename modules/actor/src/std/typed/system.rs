use alloc::{string::String, vec::Vec};

use crate::{
  core::{
    actor::Pid,
    dead_letter::DeadLetterEntry,
    error::SendError,
    event::{
      logging::LogLevel,
      stream::{
        EventStreamEvent, EventStreamShared, EventStreamSubscription, subscriber_handle as core_subscriber_handle,
      },
    },
    futures::ActorFutureShared,
    spawn::SpawnError,
    system::state::SystemStateShared,
    typed::TypedActorSystem as CoreTypedActorSystem,
  },
  std::{
    event::stream::{EventStreamSubscriberAdapter, EventStreamSubscriberShared},
    typed::{TypedProps, actor::TypedActorRef},
  },
};

type StdSubscriberHandle = EventStreamSubscriberShared;

/// Typed actor system for the standard runtime.
///
/// This is a newtype wrapper that provides std-specific convenience methods,
/// particularly for event stream operations with type conversions.
pub struct TypedActorSystem<M>
where
  M: Send + Sync + 'static, {
  inner: CoreTypedActorSystem<M>,
}

impl<M> TypedActorSystem<M>
where
  M: Send + Sync + 'static,
{
  /// Creates an empty typed actor system (for testing).
  #[must_use]
  #[cfg(any(test, feature = "test-support"))]
  pub fn new_empty() -> Self {
    Self { inner: CoreTypedActorSystem::new_empty() }
  }

  /// Creates a new typed actor system with the given guardian props.
  ///
  /// # Errors
  ///
  /// Returns an error if the guardian actor cannot be spawned.
  pub fn new(
    guardian: &TypedProps<M>,
    tick_driver_config: crate::core::scheduler::tick_driver::TickDriverConfig,
  ) -> Result<Self, SpawnError> {
    Ok(Self { inner: CoreTypedActorSystem::new(guardian.as_core(), tick_driver_config)? })
  }

  /// Returns the typed user guardian reference.
  #[must_use]
  pub fn user_guardian_ref(&self) -> TypedActorRef<M> {
    TypedActorRef::from_core(self.inner.user_guardian_ref())
  }

  /// Returns the shared system state handle.
  #[must_use]
  pub fn state(&self) -> SystemStateShared {
    self.inner.state()
  }

  /// Allocates a new pid (testing helper).
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    self.inner.allocate_pid()
  }

  /// Returns the shared event stream handle.
  #[must_use]
  pub fn event_stream(&self) -> EventStreamShared {
    self.inner.event_stream()
  }

  /// Subscribes the provided observer to the event stream.
  ///
  /// This method provides std-specific type conversion from the local
  /// `EventStreamSubscriber` trait to the core trait.
  #[must_use]
  pub fn subscribe_event_stream(&self, subscriber: &StdSubscriberHandle) -> EventStreamSubscription {
    let adapter = core_subscriber_handle(EventStreamSubscriberAdapter::new(subscriber.clone()));
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
  pub fn when_terminated(&self) -> ActorFutureShared<()> {
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
