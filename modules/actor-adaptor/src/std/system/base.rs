use alloc::{string::String, vec::Vec};

use fraktor_actor_rs::core::kernel::{
  actor::{
    Pid,
    actor_path::ActorPath,
    actor_ref::{ActorRef, dead_letter::DeadLetterEntry},
    actor_ref_provider::ActorRefResolveError,
    error::SendError,
    messaging::AskResult,
    props::Props,
    scheduler::{SchedulerBackedDelayProvider, SchedulerShared, tick_driver::TickDriverConfig},
    setup::ActorSystemConfig,
    spawn::SpawnError,
  },
  event::{
    logging::LogLevel,
    stream::{
      EventStreamEvent, EventStreamShared, EventStreamSubscriberShared, EventStreamSubscription, TickDriverSnapshot,
    },
  },
  system::{ActorSystem as CoreActorSystem, ExtendedActorSystem, state::SystemStateShared as CoreSystemStateShared},
  util::futures::ActorFutureShared,
};

#[cfg(feature = "tokio-executor")]
use crate::std::{dispatch::dispatcher::DispatcherConfig, scheduler::TickDriverConfig as StdTickDriverConfig};

#[cfg(all(test, feature = "tokio-executor"))]
mod tests;

type StdSubscriberHandle = EventStreamSubscriberShared;

/// Actor system for the standard runtime with ergonomics for standard runtime consumers.
pub struct ActorSystem {
  inner: CoreActorSystem,
}

impl ActorSystem {
  /// Creates a new actor system with default configuration.
  ///
  /// When the `tokio-executor` feature is enabled, this automatically applies:
  /// - Default tick driver with 10ms resolution
  /// - Default dispatcher using the current Tokio runtime handle
  ///
  /// # Panics
  ///
  /// Panics when `tokio-executor` is enabled and called outside a Tokio runtime context.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when the user guardian props cannot be initialised
  /// or tick driver setup fails.
  #[cfg(feature = "tokio-executor")]
  pub fn new(props: &Props) -> Result<Self, SpawnError> {
    let tick_driver = StdTickDriverConfig::default_config();
    let config = ActorSystemConfig::default()
      .with_tick_driver(tick_driver)
      .with_default_dispatcher(DispatcherConfig::default_config().into_core());
    Self::new_with_config(props, &config)
  }

  /// Creates a new actor system with an explicit tick driver configuration.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when the user guardian props cannot be initialised or tick driver setup
  /// fails.
  pub fn new_with_tick_driver(props: &Props, tick_driver_config: TickDriverConfig) -> Result<Self, SpawnError> {
    CoreActorSystem::new(props, tick_driver_config).map(Self::from_core)
  }

  /// Creates a new actor system with an explicit configuration.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::InvalidProps`] when the user guardian props cannot be
  /// initialised with the supplied configuration.
  pub fn new_with_config(props: &Props, config: &ActorSystemConfig) -> Result<Self, SpawnError> {
    CoreActorSystem::new_with_config(props, config).map(Self::from_core)
  }

  /// Creates an empty actor system without any guardian (testing helper).
  #[must_use]
  #[cfg(any(test, feature = "test-support"))]
  pub fn new_empty() -> Self {
    Self::from_core(CoreActorSystem::new_empty())
  }

  /// Constructs the wrapper from a core actor system.
  #[must_use]
  pub const fn from_core(inner: CoreActorSystem) -> Self {
    Self { inner }
  }

  /// Borrows the underlying core actor system.
  #[must_use]
  #[allow(dead_code)]
  pub const fn as_core(&self) -> &CoreActorSystem {
    &self.inner
  }

  /// Consumes the wrapper and returns the core actor system.
  #[must_use]
  pub fn into_core(self) -> CoreActorSystem {
    self.inner
  }

  /// Returns the actor reference to the user guardian.
  #[must_use]
  pub fn user_guardian_ref(&self) -> ActorRef {
    self.inner.user_guardian_ref()
  }

  /// Returns the shared system state.
  #[must_use]
  pub fn state(&self) -> CoreSystemStateShared {
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
  pub fn event_stream(&self) -> EventStreamShared {
    self.inner.event_stream()
  }

  /// Returns the last reported tick driver snapshot.
  #[must_use]
  pub fn tick_driver_snapshot(&self) -> Option<TickDriverSnapshot> {
    self.inner.tick_driver_snapshot()
  }

  /// Returns the shared scheduler handle.
  #[must_use]
  pub fn scheduler(&self) -> SchedulerShared {
    self.inner.scheduler()
  }

  /// Returns a delay provider backed by the scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider {
    self.inner.delay_provider()
  }

  /// Subscribes the provided observer to the event stream.
  #[must_use]
  pub fn subscribe_event_stream(&self, subscriber: &StdSubscriberHandle) -> EventStreamSubscription {
    self.inner.subscribe_event_stream(subscriber)
  }

  /// Returns a snapshot of recorded deadletters.
  #[must_use]
  pub fn dead_letters(&self) -> Vec<DeadLetterEntry> {
    self.inner.dead_letters()
  }

  /// Emits a log event with the specified severity.
  ///
  /// `logger_name` identifies the emitting logger. Passing `Some(...)` uses
  /// that explicit name, while `None` delegates to `self.inner.emit_log`
  /// using the default or caller-derived logger identity.
  pub fn emit_log(
    &self,
    level: LogLevel,
    message: impl Into<String>,
    origin: Option<Pid>,
    logger_name: Option<String>,
  ) {
    self.inner.emit_log(level, message, origin, logger_name)
  }

  /// Publishes a raw event to the event stream.
  pub fn publish_event(&self, event: &EventStreamEvent) {
    self.inner.publish_event(event)
  }

  /// Drains ask futures that have been fulfilled since the last check.
  #[must_use]
  pub fn drain_ready_ask_futures(&self) -> Vec<ActorFutureShared<AskResult>> {
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
  pub fn when_terminated(&self) -> ActorFutureShared<()> {
    self.inner.when_terminated()
  }

  /// Resolves an actor reference for the provided canonical or logical path.
  ///
  /// # Errors
  ///
  /// Returns [`ActorRefResolveError`] when the path cannot be resolved.
  pub fn resolve_actor_ref(&self, path: ActorPath) -> Result<ActorRef, ActorRefResolveError> {
    self.inner.resolve_actor_ref(path)
  }
}
