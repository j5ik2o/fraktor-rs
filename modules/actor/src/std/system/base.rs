use alloc::{string::String, vec::Vec};

use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

pub use crate::std::dispatcher::{DispatchExecutor, DispatchShared, Dispatcher, DispatcherConfig};
use crate::{
  core::{
    actor_prim::Pid,
    config::ActorSystemConfig,
    event_stream::{EventStreamSubscriber as CoreEventStreamSubscriber, TickDriverSnapshot},
    logging::LogLevel,
    spawn::SpawnError,
    system::{
      ActorSystemGeneric as CoreActorSystemGeneric, ExtendedActorSystemGeneric,
      SystemStateGeneric as CoreSystemStateGeneric,
    },
  },
  std::{
    actor_prim::ActorRef,
    dead_letter::DeadLetterEntry,
    error::SendError,
    event_stream::{
      EventStream, EventStreamEvent, EventStreamSubscriber, EventStreamSubscriberAdapter, EventStreamSubscription,
    },
    futures::ActorFuture,
    messaging::AnyMessage,
    props::Props,
  },
};

/// Actor system specialised for `StdToolbox` with ergonomics for standard runtime consumers.
pub struct ActorSystem {
  inner: CoreActorSystemGeneric<StdToolbox>,
}

impl ActorSystem {
  /// Creates a new actor system with the required tick driver configuration.
  ///
  /// This is the recommended way to create an actor system with minimal configuration.
  ///
  /// # Arguments
  ///
  /// * `props` - Properties for the user guardian actor
  /// * `tick_driver_config` - Tick driver configuration (required)
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when the user guardian props cannot be initialised or tick driver setup
  /// fails.
  pub fn new(
    props: &Props,
    tick_driver_config: crate::core::scheduler::TickDriverConfig<StdToolbox>,
  ) -> Result<Self, SpawnError> {
    CoreActorSystemGeneric::new(props.as_core(), tick_driver_config).map(Self::from_core)
  }

  /// Creates a new actor system with an explicit configuration.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::InvalidProps`] when the user guardian props cannot be
  /// initialised with the supplied configuration.
  pub fn new_with_config(props: &Props, config: &ActorSystemConfig<StdToolbox>) -> Result<Self, SpawnError> {
    CoreActorSystemGeneric::new_with_config(props.as_core(), config).map(Self::from_core)
  }

  /// Creates an empty actor system without any guardian (testing helper).
  #[must_use]
  pub fn new_empty() -> Self {
    Self::from_core(CoreActorSystemGeneric::new_empty())
  }

  /// Constructs the wrapper from a core actor system.
  #[must_use]
  pub const fn from_core(inner: CoreActorSystemGeneric<StdToolbox>) -> Self {
    Self { inner }
  }

  /// Borrows the underlying core actor system.
  #[must_use]
  #[allow(dead_code)]
  pub const fn as_core(&self) -> &CoreActorSystemGeneric<StdToolbox> {
    &self.inner
  }

  /// Consumes the wrapper and returns the core actor system.
  #[must_use]
  pub fn into_core(self) -> CoreActorSystemGeneric<StdToolbox> {
    self.inner
  }

  /// Returns the actor reference to the user guardian.
  #[must_use]
  pub fn user_guardian_ref(&self) -> ActorRef {
    self.inner.user_guardian_ref()
  }

  /// Returns the shared system state.
  #[must_use]
  pub fn state(&self) -> ArcShared<SystemState> {
    self.inner.state()
  }

  /// Returns an extended handle exposing privileged operations.
  #[must_use]
  pub fn extended(&self) -> ExtendedActorSystem {
    ExtendedActorSystem::new(self.inner.clone())
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

  /// Returns the last reported tick driver snapshot.
  #[must_use]
  pub fn tick_driver_snapshot(&self) -> Option<TickDriverSnapshot> {
    self.inner.tick_driver_snapshot()
  }

  /// Subscribes the provided observer to the event stream.
  #[must_use]
  pub fn subscribe_event_stream(&self, subscriber: &ArcShared<dyn EventStreamSubscriber>) -> EventStreamSubscription {
    let adapter: ArcShared<dyn CoreEventStreamSubscriber<StdToolbox>> =
      ArcShared::new(EventStreamSubscriberAdapter::new(subscriber.clone()));
    self.inner.subscribe_event_stream(&adapter)
  }

  /// Returns a snapshot of recorded deadletters.
  #[must_use]
  pub fn dead_letters(&self) -> Vec<DeadLetterEntry> {
    self.inner.dead_letters()
  }

  /// Emits a log event with the specified severity.
  pub fn emit_log(&self, level: LogLevel, message: impl Into<String>, origin: Option<Pid>) {
    self.inner.emit_log(level, message, origin)
  }

  /// Publishes a raw event to the event stream.
  pub fn publish_event(&self, event: &EventStreamEvent) {
    self.inner.publish_event(event)
  }

  /// Drains ask futures that have been fulfilled since the last check.
  #[must_use]
  pub fn drain_ready_ask_futures(&self) -> Vec<ArcShared<ActorFuture<AnyMessage>>> {
    self.inner.drain_ready_ask_futures()
  }

  /// Sends a stop signal to the user guardian and initiates system shutdown.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] when the guardian mailbox refuses the termination message.
  pub fn terminate(&self) -> Result<(), SendError> {
    self.inner.terminate()
  }

  /// Returns a future that resolves once the actor system terminates.
  #[must_use]
  pub fn when_terminated(&self) -> ArcShared<ActorFuture<()>> {
    self.inner.when_terminated()
  }
}

/// Shared system state specialised for `StdToolbox`.
pub type SystemState = CoreSystemStateGeneric<StdToolbox>;

/// Extended actor system type specialised for `StdToolbox`.
pub type ExtendedActorSystem = ExtendedActorSystemGeneric<StdToolbox>;
