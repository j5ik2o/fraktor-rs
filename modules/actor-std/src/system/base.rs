use cellactor_actor_core_rs::{
  actor_prim::Pid,
  dead_letter::DeadLetterEntryGeneric,
  logging::LogLevel,
  spawn::SpawnError,
  system::{ActorSystemGeneric as CoreActorSystemGeneric, SystemStateGeneric as CoreSystemStateGeneric},
};
use cellactor_utils_core_rs::sync::ArcShared;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

pub use crate::dispatcher::{DispatchExecutor, DispatchShared, Dispatcher, DispatcherConfig};
use crate::{
  actor_prim::ActorRef,
  error::SendError,
  event_stream::{self, EventStream, EventStreamEvent, EventStreamSubscriber, EventStreamSubscription},
  futures::ActorFuture,
  messaging::AnyMessage,
  props::Props,
};

/// Actor system specialised for `StdToolbox` with ergonomics for standard runtime consumers.
pub struct ActorSystem {
  inner: CoreActorSystemGeneric<StdToolbox>,
}

impl ActorSystem {
  /// Creates a new actor system using the provided user guardian props.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::InvalidProps`] when the user guardian props cannot be initialised.
  pub fn new(props: &Props) -> Result<Self, SpawnError> {
    CoreActorSystemGeneric::new(props.as_core()).map(Self::from_core)
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
  pub(crate) const fn as_core(&self) -> &CoreActorSystemGeneric<StdToolbox> {
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
  #[must_use]
  pub fn subscribe_event_stream(&self, subscriber: &ArcShared<dyn EventStreamSubscriber>) -> EventStreamSubscription {
    event_stream::subscribe(self, subscriber)
  }

  /// Returns a snapshot of recorded deadletters.
  #[must_use]
  pub fn deadletters(&self) -> Vec<DeadLetterEntryGeneric<StdToolbox>> {
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
