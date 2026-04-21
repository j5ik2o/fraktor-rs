//! Typed actor system wrapper.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, format, string::String, vec::Vec};
use core::{marker::PhantomData, time::Duration};

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::{
  kernel::{
    actor::{
      Address, Pid,
      actor_ref::{
        ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome,
        dead_letter::{DeadLetterEntry, DeadLetterReason},
      },
      error::SendError,
      extension::{Extension, ExtensionId},
      messaging::{AnyMessage, AskResult},
      scheduler::SchedulerBackedDelayProvider,
      setup::ActorSystemConfig,
      spawn::SpawnError,
    },
    event::{
      logging::LogLevel,
      stream::{
        ActorRefEventStreamSubscriber, EventStreamEvent, EventStreamShared, EventStreamSubscriberShared,
        EventStreamSubscription,
      },
    },
    system::{ActorSystem, TerminationSignal, state::SystemStateShared},
    util::futures::ActorFutureShared,
  },
  typed::{
    TypedActorRef, TypedActorSystemConfig, TypedActorSystemLog,
    actor::TypedChildRef,
    dispatchers::Dispatchers,
    eventstream::EventStreamCommand,
    internal::TypedSchedulerShared,
    props::TypedProps,
    receptionist::{ReceptionistCommand, SYSTEM_RECEPTIONIST_TOP_LEVEL},
    scheduler::Scheduler,
  },
};

struct IgnoreRefSender;

struct EventStreamRefEndpoint {
  actor_ref: ActorRef,
}

struct EventStreamRefSender {
  event_stream:  EventStreamShared,
  subscriptions: Vec<EventStreamActorSubscription>,
}

#[derive(Clone, Copy, Debug, Default)]
struct EventStreamRefId;

struct EventStreamActorSubscription {
  pid:          Pid,
  subscription: EventStreamSubscription,
}

struct DeadLetterRefSender {
  state: SystemStateShared,
}

const EVENT_STREAM_FACADE_PID: Pid = Pid::new(u64::MAX - 2, 0);
const DEAD_LETTER_FACADE_PID: Pid = Pid::new(u64::MAX - 1, 0);
const IGNORE_FACADE_PID: Pid = Pid::new(u64::MAX, 0);

impl EventStreamActorSubscription {
  const fn new(pid: Pid, subscription: EventStreamSubscription) -> Self {
    Self { pid, subscription }
  }
}

impl EventStreamRefEndpoint {
  const fn new(actor_ref: ActorRef) -> Self {
    Self { actor_ref }
  }

  fn actor_ref(&self) -> ActorRef {
    self.actor_ref.clone()
  }
}

impl EventStreamRefSender {
  const fn new(event_stream: EventStreamShared) -> Self {
    Self { event_stream, subscriptions: Vec::new() }
  }

  fn subscribe_actor(&mut self, subscriber: &ActorRef) {
    if self.subscriptions.iter().any(|entry| entry.pid == subscriber.pid()) {
      return;
    }
    let subscriber_handle =
      EventStreamSubscriberShared::new(Box::new(ActorRefEventStreamSubscriber::new(subscriber.clone())));
    let subscription = self.event_stream.subscribe(&subscriber_handle);
    self.subscriptions.push(EventStreamActorSubscription::new(subscriber.pid(), subscription));
  }

  fn unsubscribe_actor(&mut self, subscriber: &ActorRef) {
    if let Some(position) = self.subscriptions.iter().position(|entry| entry.pid == subscriber.pid()) {
      let removed = self.subscriptions.swap_remove(position);
      drop(removed.subscription);
    }
  }
}

impl DeadLetterRefSender {
  const fn new(state: SystemStateShared) -> Self {
    Self { state }
  }
}

impl Extension for EventStreamRefEndpoint {}

impl EventStreamRefId {
  const fn new() -> Self {
    Self
  }
}

impl ExtensionId for EventStreamRefId {
  type Ext = EventStreamRefEndpoint;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    let state = system.state();
    let actor_ref =
      ActorRef::with_system(EVENT_STREAM_FACADE_PID, EventStreamRefSender::new(system.event_stream()), &state);
    EventStreamRefEndpoint::new(actor_ref)
  }
}

impl ActorRefSender for IgnoreRefSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    Ok(SendOutcome::Delivered)
  }
}

impl ActorRefSender for EventStreamRefSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    let Some(command) = message.downcast_ref::<EventStreamCommand>() else {
      return Err(SendError::invalid_payload(message, "expected EventStreamCommand"));
    };
    match command {
      | EventStreamCommand::Publish(event) => self.event_stream.publish(event),
      | EventStreamCommand::Subscribe { subscriber } => self.subscribe_actor(subscriber),
      | EventStreamCommand::Unsubscribe { subscriber } => self.unsubscribe_actor(subscriber),
    }
    Ok(SendOutcome::Delivered)
  }
}

impl ActorRefSender for DeadLetterRefSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.state.record_dead_letter(message, DeadLetterReason::ExplicitRouting, None);
    Ok(SendOutcome::Delivered)
  }
}

/// Actor system facade that enforces a message type `M` at the API boundary.
pub struct TypedActorSystem<M>
where
  M: Send + Sync + 'static, {
  inner:            ActorSystem,
  cached_address:   Address,
  event_stream_ref: TypedActorRef<EventStreamCommand>,
  marker:           PhantomData<M>,
}

fn build_event_stream_ref(system: &ActorSystem) -> TypedActorRef<EventStreamCommand> {
  let endpoint = system.extended().register_extension(&EventStreamRefId::new());
  TypedActorRef::from_untyped(endpoint.actor_ref())
}

impl<M> TypedActorSystem<M>
where
  M: Send + Sync + 'static,
{
  /// Creates a typed actor system using the supplied configuration.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] if guardian initialization fails.
  pub fn create_with_config(guardian: &TypedProps<M>, config: ActorSystemConfig) -> Result<Self, SpawnError> {
    let inner = ActorSystem::create_with_config(guardian.to_untyped(), config)?;
    let cached_address = Address::local(inner.name());
    let event_stream_ref = build_event_stream_ref(&inner);
    Ok(Self { inner, cached_address, event_stream_ref, marker: PhantomData })
  }
}

impl<M> TypedActorSystem<M>
where
  M: Send + Sync + 'static,
{
  /// Returns the typed user guardian reference.
  #[must_use]
  pub fn user_guardian_ref(&self) -> TypedActorRef<M> {
    TypedActorRef::from_untyped(self.inner.user_guardian_ref())
  }

  /// Returns the untyped system for advanced scenarios.
  #[must_use]
  pub const fn as_untyped(&self) -> &ActorSystem {
    &self.inner
  }

  /// Consumes the typed wrapper and returns the untyped system.
  #[must_use]
  pub fn into_untyped(self) -> ActorSystem {
    self.inner
  }

  /// Returns the shared system state handle.
  #[must_use]
  pub fn state(&self) -> SystemStateShared {
    self.inner.state()
  }

  /// Returns the system receptionist reference when the bootstrap installed it.
  #[must_use]
  pub fn receptionist_ref(&self) -> Option<TypedActorRef<ReceptionistCommand>> {
    self.inner.state().extra_top_level(SYSTEM_RECEPTIONIST_TOP_LEVEL).map(TypedActorRef::from_untyped)
  }

  /// Returns the system receptionist reference.
  ///
  /// # Panics
  ///
  /// Panics if the underlying actor system was created without the system
  /// receptionist top-level actor being installed.
  #[must_use]
  pub fn receptionist(&self) -> TypedActorRef<ReceptionistCommand> {
    let Some(receptionist) = self.receptionist_ref() else {
      panic!("system receptionist must be installed during actor system bootstrap");
    };
    receptionist
  }

  /// Allocates a new pid (testing helper).
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    self.inner.allocate_pid()
  }

  /// Returns the typed event stream command endpoint.
  #[must_use]
  pub fn event_stream(&self) -> TypedActorRef<EventStreamCommand> {
    self.event_stream_ref.clone()
  }

  /// Subscribes the provided observer to the event stream.
  #[must_use]
  pub fn subscribe_event_stream(&self, subscriber: &EventStreamSubscriberShared) -> EventStreamSubscription {
    self.inner.subscribe_event_stream(subscriber)
  }

  /// Returns the dead-letter sink facade.
  #[must_use]
  pub fn dead_letters<U>(&self) -> TypedActorRef<U>
  where
    U: Send + Sync + 'static, {
    let state = self.inner.state();
    let actor_ref = ActorRef::with_system(DEAD_LETTER_FACADE_PID, DeadLetterRefSender::new(state.clone()), &state);
    TypedActorRef::from_untyped(actor_ref)
  }

  /// Returns a snapshot of recorded dead letters for diagnostics and tests.
  #[must_use]
  pub fn dead_letter_entries(&self) -> Vec<DeadLetterEntry> {
    self.inner.dead_letters()
  }

  /// Returns an actor reference that accepts and discards every message.
  #[must_use]
  pub fn ignore_ref<U>(&self) -> TypedActorRef<U>
  where
    U: Send + Sync + 'static, {
    let state = self.inner.state();
    let sender = ActorRefSenderShared::new(Box::new(IgnoreRefSender));
    TypedActorRef::from_untyped(ActorRef::from_shared(IGNORE_FACADE_PID, sender, &state))
  }

  /// Renders the current actor hierarchy for debugging.
  ///
  /// The exact format is not stable and must not be parsed.
  ///
  /// # Panics
  ///
  /// Panics if the actor system has not completed bootstrap and therefore has
  /// no root guardian registered.
  #[must_use]
  pub fn print_tree(&self) -> String {
    let state = self.inner.state();
    let Some(root_pid) = state.root_guardian_pid() else {
      panic!("actor system must be bootstrapped before print_tree");
    };
    let mut tree = String::new();
    append_tree_line(&mut tree, &state, root_pid, 0);
    tree
  }

  /// Emits a log event with the specified severity.
  pub fn emit_log(
    &self,
    level: LogLevel,
    message: impl Into<String>,
    origin: Option<Pid>,
    logger_name: Option<String>,
  ) {
    self.inner.emit_log(level, message, origin, logger_name)
  }

  /// Returns the configured actor system name.
  ///
  /// Corresponds to Pekko's `ActorSystem.name`.
  #[must_use]
  pub fn name(&self) -> String {
    self.inner.name()
  }

  /// Returns the default address of this actor system.
  ///
  /// Corresponds to Pekko's `ActorSystem.address`.
  #[must_use]
  pub fn address(&self) -> Address {
    self.cached_address.clone()
  }

  /// Returns the start time of the actor system (epoch-relative duration).
  ///
  /// Corresponds to Pekko's `ActorSystem.startTime`.
  #[must_use]
  pub const fn start_time(&self) -> Duration {
    self.inner.start_time()
  }

  /// Returns the elapsed time since the system was started.
  ///
  /// In `no_std` environments the caller must provide the current time.
  /// Corresponds to Pekko's `ActorSystem.uptime`.
  #[must_use]
  pub const fn uptime(&self, now: Duration) -> Duration {
    now.saturating_sub(self.start_time())
  }

  /// Returns the immutable config snapshot preserved by the underlying actor system.
  ///
  /// Corresponds to Pekko's `ActorSystem.settings`.
  #[must_use]
  pub fn settings(&self) -> TypedActorSystemConfig {
    self.inner.settings()
  }

  /// Emits a summary of the current system configuration to the event stream.
  ///
  /// Corresponds to Pekko's `ActorSystem.logConfiguration()`.
  pub fn log_configuration(&self) {
    let settings = self.settings();
    self.log().emit(
      LogLevel::Info,
      format!(
        "typed actor system configuration: system_name={}, start_time={:?}",
        settings.system_name(),
        settings.start_time()
      ),
    );
  }

  /// Returns the system-level log handle.
  ///
  /// Corresponds to Pekko's `ActorSystem.log`.
  #[must_use]
  pub fn log(&self) -> TypedActorSystemLog {
    TypedActorSystemLog::new(self.inner.clone())
  }

  /// Publishes a raw event to the event stream.
  pub fn publish_event(&self, event: &EventStreamEvent) {
    self.inner.publish_event(event)
  }

  /// Spawns a new top-level actor under the user guardian.
  #[allow(dead_code)]
  pub(crate) fn spawn<C>(&self, typed_props: &TypedProps<C>) -> Result<TypedChildRef<C>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let child = self.inner.spawn(typed_props.to_untyped())?;
    Ok(TypedChildRef::from_untyped(child))
  }

  /// Spawns a named actor under the `/system` guardian.
  ///
  /// This is intended for library and runtime components, matching Pekko's
  /// `systemActorOf` contract.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] if the system guardian is unavailable or the actor
  /// cannot be created.
  pub fn system_actor_of<C>(&self, typed_props: &TypedProps<C>, name: &str) -> Result<TypedActorRef<C>, SpawnError>
  where
    C: Send + Sync + 'static, {
    let named_props = typed_props.clone().map_props(|props| props.with_name(name));
    let child = self.inner.extended().spawn_system_actor(named_props.to_untyped())?;
    Ok(TypedActorRef::from_untyped(child.into_actor_ref()))
  }

  /// Returns a signal that resolves once the actor system terminates.
  #[must_use]
  pub fn when_terminated(&self) -> TerminationSignal {
    self.inner.when_terminated()
  }

  /// Returns a signal that resolves once the actor system terminates.
  ///
  /// Corresponds to Pekko's Java API alias `ActorSystem.getWhenTerminated`.
  #[must_use]
  pub fn get_when_terminated(&self) -> TerminationSignal {
    self.when_terminated()
  }

  /// Sends a stop signal to the user guardian and initiates system shutdown.
  ///
  /// # Errors
  ///
  /// Returns an error if the terminate signal cannot be sent.
  pub fn terminate(&self) -> Result<(), SendError> {
    self.inner.terminate()
  }

  /// Drains ask futures that have been fulfilled since the last check.
  #[must_use]
  pub fn drain_ready_ask_futures(&self) -> Vec<ActorFutureShared<AskResult>> {
    self.inner.drain_ready_ask_futures()
  }

  /// Wraps an existing untyped actor system so typed APIs can mirror its services.
  #[must_use]
  pub fn from_untyped(system: ActorSystem) -> Self {
    let cached_address = Address::local(system.name());
    let event_stream_ref = build_event_stream_ref(&system);
    Self { inner: system, cached_address, event_stream_ref, marker: PhantomData }
  }

  /// Returns the typed scheduler facade.
  ///
  /// Corresponds to Pekko's `ActorSystem.scheduler`.
  #[must_use]
  pub fn scheduler(&self) -> Scheduler {
    Scheduler::new(TypedSchedulerShared::new(self.inner.scheduler()))
  }

  /// Returns the raw typed scheduler shared handle for internal wiring.
  #[must_use]
  pub(crate) fn raw_scheduler(&self) -> TypedSchedulerShared {
    TypedSchedulerShared::new(self.inner.scheduler())
  }

  /// Returns the typed dispatcher lookup facade.
  ///
  /// Corresponds to Pekko's `ActorSystem.dispatchers`.
  #[must_use]
  pub fn dispatchers(&self) -> Dispatchers {
    Dispatchers::new(self.inner.state())
  }

  /// Returns a delay provider backed by the scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider {
    self.inner.delay_provider()
  }

  /// Returns the extension registered for the given identifier.
  ///
  /// Corresponds to Pekko's `ActorSystem.extension`.
  #[must_use]
  pub fn extension<E>(&self, ext_id: &E) -> Option<ArcShared<E::Ext>>
  where
    E: ExtensionId, {
    self.inner.extended().extension(ext_id)
  }

  /// Returns whether an extension has been registered.
  ///
  /// Corresponds to Pekko's `ActorSystem.hasExtension`.
  #[must_use]
  pub fn has_extension<E>(&self, ext_id: &E) -> bool
  where
    E: ExtensionId, {
    self.inner.extended().has_extension(ext_id)
  }

  /// Registers an extension if not already present (putIfAbsent semantics).
  ///
  /// Corresponds to Pekko's `ActorSystem.registerExtension`.
  pub fn register_extension<E>(&self, ext_id: &E) -> ArcShared<E::Ext>
  where
    E: ExtensionId, {
    self.inner.extended().register_extension(ext_id)
  }
}

impl<M> Clone for TypedActorSystem<M>
where
  M: Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self {
      inner:            self.inner.clone(),
      cached_address:   self.cached_address.clone(),
      event_stream_ref: self.event_stream_ref.clone(),
      marker:           PhantomData,
    }
  }
}

fn append_tree_line(tree: &mut String, state: &SystemStateShared, pid: Pid, depth: usize) {
  if !tree.is_empty() {
    tree.push('\n');
  }
  for _ in 0..depth {
    tree.push_str("  ");
  }
  let Some(path) = state.actor_path(&pid) else {
    panic!("registered actor must have a logical path");
  };
  tree.push_str(&path.to_string());

  let mut child_pids = state.child_pids(pid);
  child_pids.sort_by_key(|child_pid| {
    let Some(path) = state.actor_path(child_pid) else {
      panic!("registered child actor must have a logical path");
    };
    path.to_string()
  });
  for child_pid in child_pids {
    append_tree_line(tree, state, child_pid, depth + 1);
  }
}
