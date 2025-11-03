use cellactor_actor_core_rs::{
  actor_prim::Pid,
  deadletter::DeadletterEntry,
  logging::LogLevel,
  spawn::SpawnError,
  system::{ActorSystemGeneric, SystemState as CoreSystemState},
};
use cellactor_utils_core_rs::sync::ArcShared;
use cellactor_utils_std_rs::StdToolbox;

/// Dispatcher utilities specialised for the standard runtime.
pub mod dispatcher;

pub use dispatcher::{DispatchExecutor, DispatchHandle, Dispatcher, DispatcherConfig};

use crate::{
  actor_prim::{ActorRef, ChildRef},
  error::SendError,
  eventstream::{self, EventStream, EventStreamEvent, EventStreamSubscriber, EventStreamSubscription},
  futures::ActorFuture,
  messaging::AnyMessage,
  props::Props,
};

/// Actor system specialised for `StdToolbox` with ergonomics for standard runtime consumers.
pub struct ActorSystem {
  inner: ActorSystemGeneric<StdToolbox>,
}

impl ActorSystem {
  /// Creates a new actor system using the provided user guardian props.
  pub fn new(props: &Props) -> Result<Self, SpawnError> {
    ActorSystemGeneric::new(props.as_core()).map(Self::from_core)
  }

  /// Creates an empty actor system without any guardian (testing helper).
  #[must_use]
  pub fn new_empty() -> Self {
    Self::from_core(ActorSystemGeneric::new_empty())
  }

  /// Constructs the wrapper from a core actor system.
  #[must_use]
  pub const fn from_core(inner: ActorSystemGeneric<StdToolbox>) -> Self {
    Self { inner }
  }

  /// Borrows the underlying core actor system.
  #[must_use]
  pub(crate) const fn as_core(&self) -> &ActorSystemGeneric<StdToolbox> {
    &self.inner
  }

  /// Consumes the wrapper and returns the core actor system.
  #[must_use]
  pub fn into_core(self) -> ActorSystemGeneric<StdToolbox> {
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
    eventstream::subscribe(self, subscriber)
  }

  /// Returns a snapshot of recorded deadletters.
  #[must_use]
  pub fn deadletters(&self) -> Vec<DeadletterEntry<StdToolbox>> {
    self.inner.deadletters()
  }

  /// Emits a log event with the specified severity.
  pub fn emit_log(&self, level: LogLevel, message: impl Into<String>, origin: Option<Pid>) {
    self.inner.emit_log(level, message, origin)
  }

  /// Publishes a raw event to the event stream.
  pub fn publish_event(&self, event: &EventStreamEvent) {
    self.inner.publish_event(event)
  }

  /// Spawns a new top-level actor under the user guardian.
  pub fn spawn(&self, props: &Props) -> Result<ChildRef, SpawnError> {
    self.inner.spawn(props.as_core())
  }

  /// Spawns a new actor as a child of the specified parent.
  pub fn spawn_child(&self, parent: Pid, props: &Props) -> Result<ChildRef, SpawnError> {
    self.inner.spawn_child(parent, props.as_core())
  }

  /// Returns an actor reference for the specified pid if registered.
  #[must_use]
  pub fn actor_ref(&self, pid: Pid) -> Option<ActorRef> {
    self.inner.actor_ref(pid)
  }

  /// Returns child references supervised by the provided parent PID.
  #[must_use]
  pub fn children(&self, parent: Pid) -> Vec<ChildRef> {
    self.inner.children(parent)
  }

  /// Sends a stop signal to the specified actor.
  pub fn stop_actor(&self, pid: Pid) -> Result<(), SendError> {
    self.inner.stop_actor(pid)
  }

  /// Drains ask futures that have been fulfilled since the last check.
  #[must_use]
  pub fn drain_ready_ask_futures(&self) -> Vec<ArcShared<ActorFuture<AnyMessage>>> {
    self.inner.drain_ready_ask_futures()
  }

  /// Sends a stop signal to the user guardian and initiates system shutdown.
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
pub type SystemState = CoreSystemState<StdToolbox>;
