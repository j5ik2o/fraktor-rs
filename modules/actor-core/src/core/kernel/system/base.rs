//! Coordinates actors and infrastructure.

#[cfg(test)]
mod tests;

use alloc::{
  collections::BTreeMap,
  format,
  string::{String, ToString},
  vec::Vec,
};
use core::time::Duration;

use fraktor_utils_core_rs::core::{
  collections::queue::capabilities::{QueueCapability, QueueCapabilityRegistry},
  sync::ArcShared,
};

use super::{
  ActorSystemWeak, Blocker, ExtendedActorSystem, TerminationSignal,
  guardian::{RootGuardianActor, SystemGuardianActor, SystemGuardianProtocol},
  remote::RemotingConfig,
};
use crate::core::{
  kernel::{
    actor::{
      ActorCell, ChildRef, Pid,
      actor_path::{ActorPath, ActorPathParts, ActorPathScheme, ActorUid, GuardianKind, PathSegment},
      actor_ref::{
        ActorRef,
        dead_letter::{DeadLetterEntry, DeadLetterReason},
      },
      actor_ref_provider::ActorRefResolveError,
      actor_selection::ActorSelection,
      error::SendError,
      messaging::{AnyMessage, AskResult, system_message::SystemMessage},
      props::{MailboxRequirement, Props},
      scheduler::{SchedulerBackedDelayProvider, SchedulerShared, tick_driver::TickDriverBundle},
      setup::{ActorSystemConfig, ActorSystemSetup, CircuitBreakerConfig},
      spawn::SpawnError,
    },
    event::{
      logging::LogLevel,
      stream::{
        EventStreamEvent, EventStreamShared, EventStreamSubscriberShared, EventStreamSubscription, TickDriverSnapshot,
      },
    },
    serialization::default_serialization_extension_id,
    system::state::{SystemStateShared, system_state::SystemState},
    util::futures::ActorFutureShared,
  },
  typed::{
    ActorRefResolver, TypedActorSystemConfig, TypedProps,
    receptionist::{Receptionist, ReceptionistCommand, SYSTEM_RECEPTIONIST_TOP_LEVEL},
  },
};

const PARENT_MISSING: &str = "parent actor not found";
const CREATE_SEND_FAILED: &str = "create system message delivery failed";

/// Core runtime structure that owns registry, guardians, and spawn logic.
pub struct ActorSystem {
  state:    SystemStateShared,
  settings: TypedActorSystemConfig,
}

impl ActorSystem {
  /// Creates an actor system from an existing system state.
  #[must_use]
  pub fn from_state(state: SystemStateShared) -> Self {
    let settings = TypedActorSystemConfig::new(state.system_name(), state.start_time());
    Self { state, settings }
  }

  /// Builds and starts an actor system from a fully constructed configuration.
  ///
  /// Creates the system state from the provided [`ActorSystemConfig`], wraps it in a
  /// [`SystemStateShared`], and marks the root as started before returning. Designed as the
  /// public seam used by adaptor-level test helpers (e.g. `new_empty_actor_system_with` in
  /// `fraktor-actor-adaptor-std-rs`) that previously relied on private constructors.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when the underlying [`SystemState`] cannot be built from the
  /// supplied configuration.
  pub fn new_started_from_config(config: ActorSystemConfig) -> Result<Self, SpawnError> {
    let state = SystemState::build_from_owned_config(config)?;
    let system = Self::from_state(SystemStateShared::new(state));
    system.state.mark_root_started();
    Ok(system)
  }

  /// Creates an actor system with the provided configuration.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when guardian initialization fails.
  pub fn create_with_config(user_guardian_props: &Props, config: ActorSystemConfig) -> Result<Self, SpawnError> {
    Self::create_with_config_and(user_guardian_props, config, |_| Ok(()))
  }

  /// Creates a new actor system from a Pekko-style setup facade.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when guardian initialization or bootstrap fails.
  pub fn create_with_setup(user_guardian_props: &Props, setup: ActorSystemSetup) -> Result<Self, SpawnError> {
    Self::create_with_config(user_guardian_props, setup.into_actor_system_config())
  }

  /// Creates an actor system with configuration and a bootstrap callback.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when guardian initialization or configuration fails.
  pub fn create_with_config_and<F>(
    user_guardian_props: &Props,
    mut config: ActorSystemConfig,
    configure: F,
  ) -> Result<Self, SpawnError>
  where
    F: FnOnce(&ActorSystem) -> Result<(), SpawnError>, {
    let extension_installers = config.take_extension_installers();
    let provider_installer = config.take_provider_installer();
    let state = SystemState::build_from_owned_config(config)?;
    let system = Self::from_state(SystemStateShared::new(state));
    system.bootstrap(user_guardian_props, configure)?;

    // bootstrap 完了後に拡張とプロバイダを登録する。初期化順序上、依存リソースがこの時点で揃うため。
    if let Some(installers) = extension_installers {
      installers.install_all(&system).map_err(|e| SpawnError::from_actor_system_build_error(&e))?;
    }

    if let Some(installer) = provider_installer {
      installer.install(&system).map_err(|e| SpawnError::from_actor_system_build_error(&e))?;
    }

    system.install_default_serialization_extension();
    system.install_default_actor_ref_resolver_extension();

    system.state.mark_root_started();

    Ok(system)
  }

  /// Resolves an actor reference for the given path, injecting canonical authority when needed.
  ///
  /// # Errors
  ///
  /// Returns [`ActorRefResolveError`] when the path cannot be resolved.
  pub fn resolve_actor_ref(&self, path: ActorPath) -> Result<ActorRef, ActorRefResolveError> {
    if !self.state.has_root_started() && self.state.root_guardian_pid().is_none() {
      return Err(ActorRefResolveError::SystemNotBootstrapped);
    }
    let resolved_path = self.prepare_actor_path(path)?;
    let scheme = resolved_path.parts().scheme();
    let result = self
      .state()
      .actor_ref_provider_call_for_scheme(scheme, resolved_path)
      .ok_or(ActorRefResolveError::ProviderMissing)?;
    result.map_err(|error| ActorRefResolveError::NotFound(format!("{error:?}")))
  }

  fn prepare_actor_path(&self, path: ActorPath) -> Result<ActorPath, ActorRefResolveError> {
    let parts = path.parts().clone();
    match parts.scheme() {
      | ActorPathScheme::FraktorTcp => {
        if parts.authority_endpoint().is_none() {
          if self.state().has_partial_canonical_authority() {
            return Err(ActorRefResolveError::InvalidAuthority);
          }
          let Some((host, Some(port))) = self.state().canonical_authority_components() else {
            return Err(ActorRefResolveError::InvalidAuthority);
          };
          let updated_parts = parts.with_authority_host(host).with_authority_port(port);
          return Ok(Self::rebuild_path(updated_parts, path.segments(), path.uid()));
        }
        Ok(path)
      },
      | ActorPathScheme::Fraktor => {
        if parts.authority_endpoint().is_none() {
          if self.state().has_partial_canonical_authority() {
            return Err(ActorRefResolveError::InvalidAuthority);
          }
          if let Some((host, Some(port))) = self.state().canonical_authority_components() {
            let updated_parts =
              parts.with_scheme(ActorPathScheme::FraktorTcp).with_authority_host(host).with_authority_port(port);
            return Ok(Self::rebuild_path(updated_parts, path.segments(), path.uid()));
          }
        }
        Ok(path)
      },
    }
  }

  fn rebuild_path(parts: ActorPathParts, segments: &[PathSegment], uid: Option<ActorUid>) -> ActorPath {
    ActorPath::from_parts_and_segments(parts, segments.to_vec(), uid)
  }

  fn actor_selection_root_path(&self) -> ActorPath {
    let mut parts = ActorPathParts::local(self.state.system_name()).with_guardian(GuardianKind::User);
    if let Some((host, Some(port))) = self.state.canonical_authority_components() {
      parts = parts.with_scheme(ActorPathScheme::FraktorTcp).with_authority_host(host).with_authority_port(port);
    }
    ActorPath::from_parts(parts)
  }

  /// Returns the actor reference to the user guardian.
  ///
  /// # Panics
  ///
  /// Panics if the user guardian has not been initialised.
  #[must_use]
  pub fn user_guardian_ref(&self) -> ActorRef {
    match self.state.user_guardian() {
      | Some(cell) => cell.actor_ref(),
      | None => panic!("user guardian has not been initialised"),
    }
  }

  /// Returns the actor reference to the system guardian when available.
  #[must_use]
  pub fn system_guardian_ref(&self) -> Option<ActorRef> {
    self.state.system_guardian().map(|cell| cell.actor_ref())
  }

  /// Creates a classic actor selection rooted at the actor system.
  #[must_use]
  pub fn actor_selection(&self, path: &str) -> ActorSelection {
    ActorSelection::new(self.state(), self.actor_selection_root_path(), path.into())
  }

  /// Creates a classic actor selection anchored to the provided path.
  #[must_use]
  pub fn actor_selection_from_path(&self, path: &ActorPath) -> ActorSelection {
    ActorSelection::from_path(self.state(), path)
  }

  /// Returns the shared system state.
  #[must_use]
  pub fn state(&self) -> SystemStateShared {
    self.state.clone()
  }

  /// Creates a weak reference to this actor system.
  ///
  /// Use this when storing a reference to the actor system in components that are
  /// themselves owned by the system (such as extensions or remoting components)
  /// to avoid circular reference issues.
  #[must_use]
  pub fn downgrade(&self) -> ActorSystemWeak {
    ActorSystemWeak { state: self.state.downgrade() }
  }

  /// Returns the canonical host/port when remoting is configured.
  #[must_use]
  pub fn canonical_authority(&self) -> Option<String> {
    self.state.canonical_authority_endpoint()
  }

  /// Returns the remoting configuration when it has been configured.
  #[must_use]
  pub fn remoting_config(&self) -> Option<RemotingConfig> {
    self.state.remoting_config()
  }

  /// Returns the default circuit-breaker configuration for this actor system.
  #[must_use]
  pub fn default_circuit_breaker_config(&self) -> CircuitBreakerConfig {
    self.state.inner.with_read(|inner| inner.default_circuit_breaker_config())
  }

  /// Returns the configured named circuit-breaker overrides.
  #[must_use]
  pub fn named_circuit_breaker_config(&self) -> BTreeMap<String, CircuitBreakerConfig> {
    self.state.inner.with_read(|inner| inner.named_circuit_breaker_config())
  }

  /// Resolves circuit-breaker configuration for the provided logical id.
  #[must_use]
  pub fn circuit_breaker_config(&self, id: &str) -> CircuitBreakerConfig {
    self.state.inner.with_read(|inner| inner.circuit_breaker_config(id))
  }

  /// Returns an extended view that exposes privileged runtime operations.
  #[must_use]
  pub fn extended(&self) -> ExtendedActorSystem {
    ExtendedActorSystem::new(self.clone())
  }

  fn install_default_serialization_extension(&self) {
    let id = default_serialization_extension_id();
    if self.extended().has_extension(&id) {
      return;
    }
    let registered = self.extended().register_extension(&id);
    if let Some(existing) = self.extended().extension(&id) {
      debug_assert!(ArcShared::ptr_eq(&registered, &existing));
    }
  }

  fn install_default_actor_ref_resolver_extension(&self) {
    ActorRefResolver::install(self);
  }

  /// Allocates a new pid (testing helper).
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    self.state.allocate_pid()
  }

  /// Returns the shared event stream handle.
  #[must_use]
  pub fn event_stream(&self) -> EventStreamShared {
    self.state.event_stream()
  }

  /// Returns the shared scheduler handle.
  #[must_use]
  pub fn scheduler(&self) -> SchedulerShared {
    self.state.scheduler()
  }

  /// Returns the tick driver bundle when initialized.
  #[must_use]
  pub fn tick_driver_bundle(&self) -> TickDriverBundle {
    self.state.tick_driver_bundle()
  }

  /// Returns the last reported tick driver snapshot.
  #[must_use]
  pub fn tick_driver_snapshot(&self) -> Option<TickDriverSnapshot> {
    self.state.tick_driver_snapshot()
  }

  /// Returns a delay provider backed by the scheduler when available.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider {
    self.state.delay_provider()
  }

  /// Subscribes the provided observer to the event stream.
  #[must_use]
  pub fn subscribe_event_stream(&self, subscriber: &EventStreamSubscriberShared) -> EventStreamSubscription {
    self.state.event_stream().subscribe(subscriber)
  }

  /// Returns a snapshot of recorded dead letters.
  #[must_use]
  pub fn dead_letters(&self) -> Vec<DeadLetterEntry> {
    self.state.dead_letters()
  }

  /// Records a deadletter entry that will also be published to the event stream.
  pub fn record_dead_letter(&self, message: AnyMessage, reason: DeadLetterReason, recipient: Option<Pid>) {
    self.state.record_dead_letter(message, reason, recipient);
  }

  /// Resolves the pid registered for the provided actor path.
  #[must_use]
  pub fn pid_by_path(&self, path: &ActorPath) -> Option<Pid> {
    self.state.with_actor_path_registry(|registry| registry.pid_for(path))
  }

  /// Returns an actor reference for the provided pid when registered.
  #[must_use]
  pub fn actor_ref_by_pid(&self, pid: Pid) -> Option<ActorRef> {
    self.state.cell(&pid).map(|cell| cell.actor_ref())
  }

  /// Emits a log event with the specified severity.
  pub fn emit_log(
    &self,
    level: LogLevel,
    message: impl Into<String>,
    origin: Option<Pid>,
    logger_name: Option<String>,
  ) {
    self.state.emit_log(level, message.into(), origin, logger_name);
  }

  /// Returns `true` when the configured logging filter would accept events of the given `level`.
  ///
  /// Callers that build log payloads lazily should use this to skip expensive
  /// argument evaluation when the level is disabled.
  #[must_use]
  pub fn is_log_level_enabled(&self, level: LogLevel) -> bool {
    self.state.is_log_level_enabled(level)
  }

  /// Returns the configured actor system name.
  ///
  /// Corresponds to Pekko's `ActorSystem.name`.
  #[must_use]
  pub fn name(&self) -> String {
    self.state.system_name()
  }

  /// Returns the immutable settings snapshot preserved by this actor system.
  #[must_use]
  pub fn settings(&self) -> TypedActorSystemConfig {
    self.settings.clone()
  }

  /// Returns the start time of the actor system (epoch-relative duration).
  ///
  /// Corresponds to Pekko's `ActorSystem.startTime`.
  #[must_use]
  pub const fn start_time(&self) -> Duration {
    self.state.start_time()
  }

  /// Publishes a raw event to the event stream.
  pub fn publish_event(&self, event: &EventStreamEvent) {
    self.state.publish_event(event);
  }

  /// Spawns a new top-level actor under the user guardian.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::SystemUnavailable`] when the guardian is missing.
  pub(crate) fn spawn(&self, props: &Props) -> Result<ChildRef, SpawnError> {
    let guardian_pid = self.state.user_guardian_pid().ok_or_else(SpawnError::system_unavailable)?;
    self.spawn_child(guardian_pid, props)
  }

  /// Spawns a new top-level actor under the user guardian.
  ///
  /// Corresponds to classic `ActorRefFactory.actorOf(props)`.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when the actor cannot be created.
  pub fn actor_of(&self, props: &Props) -> Result<ChildRef, SpawnError> {
    self.spawn(props)
  }

  /// Spawns a detached actor without requiring bootstrap guardians.
  ///
  /// This is intended for internal support actors that still need a real actor
  /// cell in empty test systems.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when the actor cannot be created.
  pub(crate) fn spawn_detached(&self, props: &Props) -> Result<ChildRef, SpawnError> {
    self.spawn_with_parent(None, props)
  }

  /// Spawns a new named top-level actor under the user guardian.
  ///
  /// Corresponds to classic `ActorRefFactory.actorOf(props, name)`.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when the actor cannot be created, including duplicate names.
  pub fn actor_of_named(&self, props: &Props, name: &str) -> Result<ChildRef, SpawnError> {
    self.spawn(&props.clone().with_name(name))
  }

  /// Spawns a new actor under the system guardian (internal use only).
  pub(crate) fn system_actor_of(&self, props: &Props) -> Result<ChildRef, SpawnError> {
    let guardian_pid = self.state.system_guardian_pid().ok_or_else(SpawnError::system_unavailable)?;
    self.spawn_child(guardian_pid, props)
  }

  /// Spawns a new actor as a child of the specified parent.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::InvalidProps`] when the parent pid is unknown.
  pub(crate) fn spawn_child(&self, parent: Pid, props: &Props) -> Result<ChildRef, SpawnError> {
    if !self.state.has_root_started() && self.state.root_guardian_pid().is_none() {
      return Err(SpawnError::system_not_bootstrapped());
    }
    if self.state.cell(&parent).is_none() {
      return Err(SpawnError::invalid_props(PARENT_MISSING));
    }
    self.spawn_with_parent(Some(parent), props)
  }

  /// Returns an [`ActorRef`] for the specified pid if the actor is registered.
  #[must_use]
  pub(crate) fn actor_ref(&self, pid: Pid) -> Option<ActorRef> {
    self.state.cell(&pid).map(|cell| cell.actor_ref())
  }

  /// Returns child references supervised by the provided parent PID.
  #[must_use]
  pub(crate) fn children(&self, parent: Pid) -> Vec<ChildRef> {
    let system = self.state.clone();
    self
      .state
      .child_pids(parent)
      .into_iter()
      .filter_map(|pid| self.state.cell(&pid).map(|cell| ChildRef::new(cell.actor_ref(), system.clone())))
      .collect()
  }

  /// Sends a stop signal to the specified actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the stop message cannot be enqueued.
  pub(crate) fn stop_actor(&self, pid: Pid) -> Result<(), SendError> {
    self.state.send_system_message(pid, SystemMessage::Stop)
  }

  /// Sends a stop signal to the specified actor reference.
  ///
  /// Corresponds to classic `ActorRefFactory.stop(actor)`.
  ///
  /// # Errors
  ///
  /// Returns an error if the stop message cannot be enqueued.
  pub fn stop(&self, actor: &ActorRef) -> Result<(), SendError> {
    self.stop_actor(actor.pid())
  }

  /// Drains ask futures that have been fulfilled since the last check.
  #[must_use]
  pub fn drain_ready_ask_futures(&self) -> Vec<ActorFutureShared<AskResult>> {
    self.state.drain_ready_ask_futures()
  }

  /// Sends a stop signal to the user guardian and initiates system shutdown.
  ///
  /// # Errors
  ///
  /// Returns an error if the guardian mailbox rejects the stop request.
  pub fn terminate(&self) -> Result<(), SendError> {
    if self.state.is_terminated() {
      return Ok(());
    }

    if self.state.begin_termination() {
      let _summary = self.state.shutdown_scheduler();
      if let Some(root_pid) = self.state.root_guardian_pid() {
        if let Some(user_pid) = self.state.user_guardian_pid() {
          return self.state.send_system_message(root_pid, SystemMessage::StopChild(user_pid));
        }
        self.state.clone().mark_terminated();
        return Ok(());
      }
      if let Some(user_pid) = self.state.user_guardian_pid() {
        return self.state.send_system_message(user_pid, SystemMessage::Stop);
      }
      self.state.clone().mark_terminated();
      Ok(())
    } else {
      self.force_termination_hooks()?;
      Ok(())
    }
  }

  /// Returns a signal that resolves once the actor system terminates.
  #[must_use]
  pub fn when_terminated(&self) -> TerminationSignal {
    self.state.termination_signal()
  }

  /// Blocks the current thread until the actor system has fully terminated.
  ///
  /// Uses the provided [`Blocker`](super::Blocker) to avoid busy-wait spin loops.
  pub fn run_until_terminated(&self, blocker: &dyn Blocker) {
    self.when_terminated().wait_blocking(blocker);
  }

  fn spawn_with_parent(&self, parent: Option<Pid>, props: &Props) -> Result<ChildRef, SpawnError> {
    let pid = self.state.allocate_pid();
    let name = self.state.assign_name(parent, props.name(), pid)?;
    let cell = self.build_cell_for_spawn(pid, parent, name, props)?;

    // AC-H4 TOCTOU-safe order: registration must complete before the `Create`
    // handshake so that if the child fails in `pre_start`, the parent already
    // sees the child in `children_state` and in its `state.watching` set. This
    // guarantees that the subsequent `DeathWatchNotification` drives
    // `finish_recreate` / `finish_terminate` without dropping state-change
    // transitions.
    self.state.register_cell(cell.clone());
    if let Some(parent_pid) = parent {
      self.state.register_child(parent_pid, pid);
      self.install_supervision_watch(parent_pid, pid, &cell);
    }
    self.perform_create_handshake(parent, pid, &cell)?;

    Ok(ChildRef::new(cell.actor_ref(), self.state.clone()))
  }

  /// Installs the bidirectional supervision watch between `parent` and the
  /// newly registered child cell.
  ///
  /// The `child_cell.state.watchers` gains `(parent, Supervision)` so the
  /// child's `notify_watchers_on_stop` reaches the parent. The parent cell's
  /// `state.watching` gains `(pid, Supervision)` so that
  /// `handle_death_watch_notification` passes the `watching_contains_pid`
  /// gate and drives `finish_recreate`. Both entries are removed by
  /// [`Self::rollback_spawn`] if the `Create` handshake subsequently fails.
  ///
  /// TOCTOU-safe: when the parent cell has already been released, the
  /// child-side registration is skipped as well so that no one-sided stale
  /// watcher entry survives.
  fn install_supervision_watch(&self, parent_pid: Pid, child_pid: Pid, child_cell: &ArcShared<ActorCell>) {
    let Some(parent_cell) = self.state.cell(&parent_pid) else {
      return;
    };
    child_cell.register_supervision_watcher(parent_pid);
    parent_cell.register_supervision_watching(child_pid);
  }

  fn bootstrap<F>(&self, user_guardian_props: &Props, configure: F) -> Result<(), SpawnError>
  where
    F: FnOnce(&ActorSystem) -> Result<(), SpawnError>, {
    let root_props = Props::from_fn(RootGuardianActor::new).with_name("root");
    let root_cell = self.spawn_root_guardian_cell(&root_props)?;
    let root_pid = root_cell.pid();
    self.state.set_root_guardian(&root_cell);

    let user_guardian = self.spawn_child(root_pid, user_guardian_props)?;
    if let Some(cell) = self.state.cell(&user_guardian.pid()) {
      self.state.set_user_guardian(&cell);
    } else {
      return Err(SpawnError::invalid_props("user guardian unavailable"));
    }

    let user_guardian_ref = user_guardian.actor_ref();
    let system_props = Props::from_fn({
      let user_guardian_ref = user_guardian_ref.clone();
      move || SystemGuardianActor::new(user_guardian_ref.clone())
    })
    .with_name("system");

    let system_guardian = self.spawn_child(root_pid, &system_props)?;
    if let Some(cell) = self.state.cell(&system_guardian.pid()) {
      self.state.set_system_guardian(&cell);
    }

    let receptionist_props =
      TypedProps::<ReceptionistCommand>::from_behavior_factory(Receptionist::behavior).into_untyped();
    let receptionist_props = receptionist_props.with_name(SYSTEM_RECEPTIONIST_TOP_LEVEL);
    let receptionist = self.spawn_child(system_guardian.pid(), &receptionist_props)?;
    let receptionist_pid = receptionist.pid();
    let receptionist_ref = receptionist.into_actor_ref();
    if let Err(error) = self.extended().register_extra_top_level(SYSTEM_RECEPTIONIST_TOP_LEVEL, receptionist_ref) {
      if let Some(cell) = self.state.cell(&receptionist_pid) {
        self.rollback_spawn(Some(system_guardian.pid()), &cell, receptionist_pid);
      }
      return Err(SpawnError::SystemBuildError(format!("system receptionist registration failed: {error:?}")));
    }

    configure(self)?;

    if let Err(error) = self.perform_create_handshake(None, root_pid, &root_cell) {
      self.rollback_spawn(None, &root_cell, root_pid);
      return Err(error);
    }
    Ok(())
  }

  fn spawn_root_guardian_cell(&self, props: &Props) -> Result<ArcShared<ActorCell>, SpawnError> {
    let pid = self.state.allocate_pid();
    let name = self.state.assign_name(None, props.name(), pid)?;
    let cell = self.build_cell_for_spawn(pid, None, name, props)?;
    self.state.register_cell(cell.clone());
    Ok(cell)
  }

  fn build_cell_for_spawn(
    &self,
    pid: Pid,
    parent: Option<Pid>,
    name: String,
    props: &Props,
  ) -> Result<ArcShared<ActorCell>, SpawnError> {
    self.ensure_mailbox_requirements(props)?;
    ActorCell::create(self.state.clone(), pid, parent, name, props)
  }

  fn ensure_mailbox_requirements(&self, props: &Props) -> Result<(), SpawnError> {
    // Requirements come from the factory that will actually be used at spawn
    // time: the registered factory when `mailbox_id` is set, otherwise the
    // inline `MailboxConfig` carried by the props.
    if let Some(mailbox_id) = props.mailbox_id() {
      let factory =
        self.state.resolve_mailbox(mailbox_id).map_err(|error| SpawnError::invalid_props(error.to_string()))?;
      Self::ensure_requirements_from(&factory.requirement(), &factory.capabilities())
    } else {
      let config = props.mailbox_config();
      Self::ensure_requirements_from(&config.requirement(), &config.capabilities())
    }
  }

  fn ensure_requirements_from(
    requirement: &MailboxRequirement,
    registry: &QueueCapabilityRegistry,
  ) -> Result<(), SpawnError> {
    requirement.ensure_supported(registry).map_err(|error| {
      let reason = Self::missing_capability_reason(error.missing());
      SpawnError::invalid_props(reason)
    })
  }

  const fn missing_capability_reason(capability: QueueCapability) -> &'static str {
    match capability {
      | QueueCapability::Mpsc => "mailbox requires MPSC capability",
      | QueueCapability::Deque => "mailbox requires deque capability",
      | QueueCapability::BlockingFuture => "mailbox requires blocking-future capability",
      | QueueCapability::ControlAware => "mailbox requires control-aware capability",
    }
  }

  fn perform_create_handshake(
    &self,
    parent: Option<Pid>,
    pid: Pid,
    cell: &ArcShared<ActorCell>,
  ) -> Result<(), SpawnError> {
    if let Err(error) = self.state.send_system_message(pid, SystemMessage::Create) {
      self.state.record_send_error(Some(pid), &error);
      self.rollback_spawn(parent, cell, pid);
      return Err(SpawnError::invalid_props(CREATE_SEND_FAILED));
    }

    Ok(())
  }

  fn rollback_spawn(&self, parent: Option<Pid>, cell: &ArcShared<ActorCell>, pid: Pid) {
    // Order matters. `rollback_spawn` runs when the spawn handshake failed
    // before the child ever started emitting `DeathWatchNotification`s, so
    // `children_state` cleanup cannot rely on the usual notification path
    // and has to happen here synchronously. `ActorCell::unregister_child`
    // short-circuits while a supervision watch is live (because the normal
    // restart flow needs the parent `DeathWatchNotification` handler to
    // observe the state change), so the supervision watching entry must be
    // torn down *first*. With the watch gone, `unregister_child` removes the
    // container entry, and `remove_cell` drops the child cell entirely.
    if let Some(parent_pid) = parent
      && let Some(parent_cell) = self.state.cell(&parent_pid)
    {
      parent_cell.unregister_supervision_watching(pid);
    }
    self.state.release_name(parent, cell.name());
    self.state.remove_cell(&pid);
    if let Some(parent_pid) = parent {
      self.state.unregister_child(Some(parent_pid), pid);
    }
  }

  fn force_termination_hooks(&self) -> Result<(), SendError> {
    if let Some(system_pid) = self.state.system_guardian_pid()
      && let Some(mut system_ref) = self.actor_ref(system_pid)
    {
      system_ref.try_tell(AnyMessage::new(SystemGuardianProtocol::ForceTerminateHooks))?;
    }
    Ok(())
  }
}

impl Clone for ActorSystem {
  fn clone(&self) -> Self {
    Self { state: self.state.clone(), settings: self.settings.clone() }
  }
}

unsafe impl Send for ActorSystem {}
unsafe impl Sync for ActorSystem {}
