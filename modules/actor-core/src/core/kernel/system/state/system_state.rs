//! Shared, mutable state owned by the actor system.

#[cfg(test)]
mod tests;

mod path_identity;

use alloc::{
  borrow::ToOwned,
  boxed::Box,
  collections::VecDeque,
  format,
  string::{String, ToString},
  vec::Vec,
};
use core::{
  any::{Any, TypeId},
  time::Duration,
};

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess};
use portable_atomic::{AtomicBool, AtomicU64, Ordering};

use self::path_identity::{DEFAULT_QUARANTINE_DURATION, PathIdentity};
use super::{
  super::termination_state::TerminationState, ActorPathRegistry, ActorRefProvider, ActorRefProviderCaller,
  ActorRefProviderCallers, ActorRefProviderHandleShared, ActorRefProviders, AskFutures, AuthorityState, CellsShared,
  Extensions, ExtraTopLevels, GuardianKind, GuardiansState, Registries, RemoteAuthorityError, RemoteAuthorityRegistry,
  RemoteWatchHookDynShared, RemotingConfig, TempActors,
};
use crate::core::kernel::{
  actor::{
    ActorCell, Pid,
    actor_path::{ActorPath, ActorPathParser, ActorPathScheme, GuardianKind as PathGuardianKind},
    actor_ref::{
      ActorRef,
      dead_letter::{DeadLetterEntry, DeadLetterShared},
    },
    deploy::Deployer,
    error::{ActorError, SendError},
    messaging::{
      AnyMessage, AskResult,
      system_message::{FailurePayload, SystemMessage},
    },
    props::MailboxConfig,
    scheduler::{
      SchedulerBackedDelayProvider, SchedulerConfig, SchedulerContext, SchedulerShared,
      task_run::TaskRunSummary,
      tick_driver::{
        TickDriverBundle, TickDriverControl, TickDriverControlShared, TickDriverHandle, TickDriverKind,
        TickDriverProvisioningContext, TickExecutorSignal, TickFeed, next_tick_driver_id,
      },
    },
    spawn::{NameRegistryError, SpawnError},
    supervision::SupervisorDirective,
  },
  dispatch::{
    dispatcher::{Dispatchers, MessageDispatcherShared},
    mailbox::{MailboxRegistryError, Mailboxes, MessageQueue},
  },
  event::{
    logging::{LogEvent, LogLevel},
    stream::{
      EventStream, EventStreamEvent, EventStreamShared, RemoteAuthorityEvent,
      TickDriverSnapshot,
    },
  },
  system::{RegisterExtraTopLevelError, ReservationPolicy},
  util::futures::ActorFutureShared,
};

mod failure_outcome;

pub(crate) use failure_outcome::FailureOutcome;

use crate::core::kernel::actor::setup::ActorSystemConfig;

const RESERVED_TOP_LEVEL: [&str; 4] = ["user", "system", "temp", "deadLetters"];

/// Captures global actor system state.
pub struct SystemState {
  next_pid: AtomicU64,
  clock: AtomicU64,
  cells: CellsShared,
  registries: Registries,
  guardians: GuardiansState,
  root_guardian_alive: AtomicBool,
  system_guardian_alive: AtomicBool,
  user_guardian_alive: AtomicBool,
  ask_futures: AskFutures,
  termination_state: ArcShared<TerminationState>,
  root_started: AtomicBool,
  event_stream: EventStreamShared,
  dead_letter: DeadLetterShared,
  extra_top_levels: ExtraTopLevels,
  temp_actors: TempActors,
  temp_counter: AtomicU64,
  failure_total: AtomicU64,
  failure_restart_total: AtomicU64,
  failure_stop_total: AtomicU64,
  failure_escalate_total: AtomicU64,
  failure_resume_total: AtomicU64,
  failure_inflight: AtomicU64,
  extensions: Extensions,
  actor_ref_providers: ActorRefProviders,
  actor_ref_provider_callers_by_scheme: ActorRefProviderCallers,
  remote_watch_hook: RemoteWatchHookDynShared,
  dispatchers: Dispatchers,
  mailboxes: Mailboxes,
  deployer: Deployer,
  path_identity: PathIdentity,
  actor_path_registry: ActorPathRegistry,
  remote_authority_registry: RemoteAuthorityRegistry,
  scheduler_context: SchedulerContext,
  tick_driver_snapshot: Option<TickDriverSnapshot>,
  tick_driver_bundle: TickDriverBundle,
  start_time: Duration,
}

impl SystemState {
  /// Creates a fresh state container without any registered actors.
  #[must_use]
  pub fn new() -> Self {
    let config = ActorSystemConfig::default();
    const DEAD_LETTER_CAPACITY: usize = 512;
    const EVENT_STREAM_CAPACITY: usize = 256;
    let event_stream_shared = EventStreamShared::new(EventStream::with_capacity(EVENT_STREAM_CAPACITY));
    let dead_letter_shared = DeadLetterShared::with_capacity(event_stream_shared.clone(), DEAD_LETTER_CAPACITY);
    let dispatchers = config.dispatchers().clone();
    let mailboxes = config.mailboxes().clone();
    let scheduler_config = *config.scheduler_config();
    let scheduler_context = SchedulerContext::with_event_stream(scheduler_config, event_stream_shared.clone());
    let tick_driver_bundle =
      Self::default_tick_driver_bundle(scheduler_config.resolution());
    Self {
      next_pid: AtomicU64::new(0),
      clock: AtomicU64::new(0),
      cells: CellsShared::default(),
      registries: Registries::default(),
      guardians: GuardiansState::default(),
      root_guardian_alive: AtomicBool::new(false),
      system_guardian_alive: AtomicBool::new(false),
      user_guardian_alive: AtomicBool::new(false),
      ask_futures: AskFutures::default(),
      termination_state: ArcShared::new(TerminationState::new()),
      root_started: AtomicBool::new(false),
      event_stream: event_stream_shared,
      dead_letter: dead_letter_shared,
      extra_top_levels: ExtraTopLevels::default(),
      temp_actors: TempActors::default(),
      temp_counter: AtomicU64::new(0),
      failure_total: AtomicU64::new(0),
      failure_restart_total: AtomicU64::new(0),
      failure_stop_total: AtomicU64::new(0),
      failure_escalate_total: AtomicU64::new(0),
      failure_resume_total: AtomicU64::new(0),
      failure_inflight: AtomicU64::new(0),
      extensions: Extensions::default(),
      actor_ref_providers: ActorRefProviders::default(),
      remote_watch_hook: RemoteWatchHookDynShared::noop(),
      dispatchers,
      mailboxes,
      deployer: Deployer::default(),
      path_identity: PathIdentity::default(),
      actor_path_registry: ActorPathRegistry::default(),
      remote_authority_registry: RemoteAuthorityRegistry::default(),
      actor_ref_provider_callers_by_scheme: ActorRefProviderCallers::default(),
      scheduler_context,
      tick_driver_snapshot: None,
      tick_driver_bundle,
      start_time: Duration::ZERO,
    }
  }

  pub(crate) fn build_from_config(config: &ActorSystemConfig) -> Result<Self, SpawnError> {
    use crate::core::kernel::actor::scheduler::tick_driver::TickDriverBootstrap;

    const DEAD_LETTER_CAPACITY: usize = 512;
    const EVENT_STREAM_CAPACITY: usize = 256;
    let event_stream = EventStreamShared::new(EventStream::with_capacity(EVENT_STREAM_CAPACITY));
    let dead_letter = DeadLetterShared::with_capacity(event_stream.clone(), DEAD_LETTER_CAPACITY);
    let mut dispatchers = Dispatchers::new();
    dispatchers.ensure_default_inline();
    let mut mailboxes = Mailboxes::new();
    mailboxes.ensure_default();
    let scheduler_config = SchedulerConfig::default();
    let scheduler_context = SchedulerContext::with_event_stream(scheduler_config, event_stream.clone());
    let tick_driver_bundle =
      Self::default_tick_driver_bundle(scheduler_config.resolution());
    let mut state = Self {
      next_pid: AtomicU64::new(0),
      clock: AtomicU64::new(0),
      cells: CellsShared::default(),
      registries: Registries::default(),
      guardians: GuardiansState::default(),
      root_guardian_alive: AtomicBool::new(false),
      system_guardian_alive: AtomicBool::new(false),
      user_guardian_alive: AtomicBool::new(false),
      ask_futures: AskFutures::default(),
      termination_state: ArcShared::new(TerminationState::new()),
      root_started: AtomicBool::new(false),
      event_stream,
      dead_letter,
      extra_top_levels: ExtraTopLevels::default(),
      temp_actors: TempActors::default(),
      temp_counter: AtomicU64::new(0),
      failure_total: AtomicU64::new(0),
      failure_restart_total: AtomicU64::new(0),
      failure_stop_total: AtomicU64::new(0),
      failure_escalate_total: AtomicU64::new(0),
      failure_resume_total: AtomicU64::new(0),
      failure_inflight: AtomicU64::new(0),
      extensions: Extensions::default(),
      actor_ref_providers: ActorRefProviders::default(),
      remote_watch_hook: RemoteWatchHookDynShared::noop(),
      dispatchers,
      mailboxes,
      deployer: Deployer::default(),
      path_identity: PathIdentity::default(),
      actor_path_registry: ActorPathRegistry::default(),
      remote_authority_registry: RemoteAuthorityRegistry::default(),
      actor_ref_provider_callers_by_scheme: ActorRefProviderCallers::default(),
      scheduler_context,
      tick_driver_snapshot: None,
      tick_driver_bundle,
      start_time: Duration::ZERO,
    };
    state.start_time = config.start_time().unwrap_or_else(|| state.monotonic_now());
    state.apply_actor_system_config(config);

    let event_stream = state.event_stream();
    let scheduler_config = *config.scheduler_config();
    #[cfg(any(test, feature = "test-support"))]
    let scheduler_config = if let Some(tick_driver_config) = config.tick_driver_config()
      && matches!(
        tick_driver_config,
        crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::ManualTest(_)
      )
      && !scheduler_config.runner_api_enabled()
    {
      scheduler_config.with_runner_api_enabled(true)
    } else {
      scheduler_config
    };

    let context = SchedulerContext::with_event_stream(scheduler_config, event_stream);
    let provisioning = TickDriverProvisioningContext::from_scheduler_context(&context);
    state.scheduler_context = context;

    let tick_driver_config = config
      .tick_driver_config()
      .ok_or_else(|| SpawnError::SystemBuildError("tick driver configuration is required".into()))?;
    let (runtime, snapshot) =
      TickDriverBootstrap::provision(tick_driver_config, &provisioning)
        .map_err(|error| SpawnError::SystemBuildError(format!("tick driver provisioning failed: {error}")))?;
    state.tick_driver_bundle = runtime;
    state.tick_driver_snapshot = Some(snapshot);

    Ok(state)
  }

  fn default_tick_driver_bundle(resolution: Duration) -> TickDriverBundle {
    struct NoopDriverControl;

    impl TickDriverControl for NoopDriverControl {
      fn shutdown(&self) {}
    }

    let signal = TickExecutorSignal::new();
    let feed = TickFeed::new(resolution, 1, signal);
    let control = TickDriverControlShared::new(Box::new(NoopDriverControl));
    let handle = TickDriverHandle::new(next_tick_driver_id(), TickDriverKind::Auto, resolution, control);
    TickDriverBundle::new(handle, feed)
  }

  /// Allocates a new unique [`Pid`] for an actor.
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    let value = self.next_pid.fetch_add(1, Ordering::Relaxed) + 1;
    Pid::new(value, 0)
  }

  /// Applies the actor system configuration (system name, remoting settings).
  pub fn apply_actor_system_config(&mut self, config: &ActorSystemConfig) {
    self.path_identity.system_name = config.system_name().to_string();
    self.path_identity.guardian_kind = config.default_guardian();
    self.dispatchers = config.dispatchers().clone();
    self.mailboxes = config.mailboxes().clone();
    if let Some(remoting) = config.remoting_config() {
      self.path_identity.canonical_host = Some(remoting.canonical_host().to_string());
      self.path_identity.canonical_port = remoting.canonical_port();
      self.path_identity.quarantine_duration = remoting.quarantine_duration();
    } else {
      self.path_identity.canonical_host = None;
      self.path_identity.canonical_port = None;
      self.path_identity.quarantine_duration = DEFAULT_QUARANTINE_DURATION;
    }

    if let Some(start_time) = config.start_time() {
      self.start_time = start_time;
    }

    let policy = ReservationPolicy::with_quarantine_duration(self.default_quarantine_duration());
    self.actor_path_registry.set_policy(policy);
  }

  fn identity_snapshot(&self) -> PathIdentity {
    self.path_identity.clone()
  }

  const fn default_quarantine_duration(&self) -> Duration {
    self.path_identity.quarantine_duration
  }

  /// Returns the configured canonical host/port pair when remoting is enabled and complete.
  pub fn canonical_authority_components(&self) -> Option<(String, Option<u16>)> {
    match (&self.path_identity.canonical_host, self.path_identity.canonical_port) {
      | (Some(host), Some(port)) => Some((host.clone(), Some(port))),
      | _ => None,
    }
  }

  /// Returns true when canonical_host is set but canonical_port is missing.
  pub const fn has_partial_canonical_authority(&self) -> bool {
    self.path_identity.canonical_host.is_some() && self.path_identity.canonical_port.is_none()
  }

  /// Returns the canonical authority string (`host[:port]`) when available.
  pub fn canonical_authority_endpoint(&self) -> Option<String> {
    self.canonical_authority_components().map(|(host, port)| match port {
      | Some(port) => format!("{host}:{port}"),
      | None => host,
    })
  }

  /// Returns the configured actor system name.
  #[must_use]
  pub fn system_name(&self) -> String {
    self.path_identity.system_name.clone()
  }

  /// Returns a snapshot of the deployer registry.
  #[must_use]
  pub fn deployer(&self) -> Deployer {
    self.deployer.clone()
  }

  /// Returns the start time of the actor system (epoch-relative duration).
  ///
  /// Corresponds to Pekko's `ActorSystem.startTime`.
  #[must_use]
  pub const fn start_time(&self) -> Duration {
    self.start_time
  }

  #[must_use]
  pub(crate) const fn path_guardian_kind(&self) -> PathGuardianKind {
    self.path_identity.guardian_kind
  }

  #[must_use]
  pub(crate) fn canonical_host(&self) -> Option<String> {
    self.path_identity.canonical_host.clone()
  }

  #[must_use]
  pub(crate) const fn canonical_port(&self) -> Option<u16> {
    self.path_identity.canonical_port
  }

  #[must_use]
  pub(crate) const fn quarantine_duration(&self) -> Duration {
    self.path_identity.quarantine_duration
  }

  fn publish_remote_authority_event(&self, authority: String, state: AuthorityState) {
    let event = RemoteAuthorityEvent::new(authority, state);
    self.event_stream.publish(&EventStreamEvent::RemoteAuthority(event));
  }

  /// Retrieves an actor cell by pid.
  #[must_use]
  pub(crate) fn cell(&self, pid: &Pid) -> Option<ArcShared<ActorCell>> {
    self.cells.with_read(|cells| cells.get(pid))
  }

  /// Returns the shared cell registry handle.
  #[must_use]
  pub(crate) fn cells_handle(&self) -> CellsShared {
    self.cells.clone()
  }

  /// Binds an actor name within its parent's scope.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] if the name assignment fails.
  pub(crate) fn assign_name(
    &mut self,
    parent: Option<Pid>,
    hint: Option<&str>,
    pid: Pid,
  ) -> Result<String, SpawnError> {
    let registry = self.registries.entry_or_insert(parent);

    match hint {
      | Some(name) => {
        registry.register(name, pid).map_err(|error| match error {
          | NameRegistryError::Duplicate(existing) => SpawnError::name_conflict(existing),
        })?;
        Ok(String::from(name))
      },
      | None => {
        let generated = registry.generate_anonymous(pid);
        registry.register(&generated, pid).map_err(|error| match error {
          | NameRegistryError::Duplicate(existing) => SpawnError::name_conflict(existing),
        })?;
        Ok(generated)
      },
    }
  }

  /// Releases the association between a name and its pid in the registry.
  pub(crate) fn release_name(&mut self, parent: Option<Pid>, name: &str) {
    if let Some(registry) = self.registries.get_mut(&parent) {
      registry.remove(name);
    }
  }

  pub(crate) fn register_ask_future(&mut self, future: ActorFutureShared<AskResult>) {
    self.ask_futures.push(future);
  }

  pub(crate) fn register_actor_path(&mut self, pid: Pid, path: &ActorPath) {
    self.actor_path_registry.register(pid, path);
  }

  #[must_use]
  pub(crate) fn drain_ready_ask_futures(&mut self) -> Vec<ActorFutureShared<AskResult>> {
    self.ask_futures.drain_ready()
  }

  #[must_use]
  pub(crate) fn register_temp_actor(&mut self, actor: ActorRef) -> String {
    let name = self.next_temp_actor_name();
    let pid = actor.pid();
    self.temp_actors.insert(name.clone(), actor);
    let mut path = ActorPath::root_with_guardian(self.path_guardian_kind());
    path = path.child("temp").child(&name);
    self.actor_path_registry.register(pid, &path);
    name
  }

  pub(crate) fn unregister_temp_actor(&mut self, name: &str) {
    if let Some(actor) = self.temp_actors.remove(name) {
      self.actor_path_registry.unregister(&actor.pid());
    }
  }

  pub(crate) fn unregister_temp_actor_by_pid(&mut self, pid: &Pid) {
    if let Some((_name, actor)) = self.temp_actors.remove_by_pid(pid) {
      self.actor_path_registry.unregister(&actor.pid());
    }
  }

  #[must_use]
  pub(crate) fn temp_actor(&self, name: &str) -> Option<ActorRef> {
    self.temp_actors.get(name)
  }

  /// Returns the shared remote watch hook handle.
  #[must_use]
  pub(crate) fn remote_watch_hook_handle(&self) -> RemoteWatchHookDynShared {
    self.remote_watch_hook.clone()
  }

  /// Registers the root guardian PID.
  pub(crate) fn set_root_guardian(&mut self, cell: &ArcShared<ActorCell>) {
    self.guardians.register(GuardianKind::Root, cell.pid());
    self.root_guardian_alive.store(true, Ordering::Release);
  }

  #[cfg(any(test, feature = "test-support"))]
  pub(crate) fn register_guardian_pid(&mut self, kind: GuardianKind, pid: Pid) {
    self.guardians.register(kind, pid);
    self.guardian_alive_flag(kind).store(true, Ordering::Release);
  }

  /// Registers the system guardian PID.
  pub(crate) fn set_system_guardian(&mut self, cell: &ArcShared<ActorCell>) {
    self.guardians.register(GuardianKind::System, cell.pid());
    self.system_guardian_alive.store(true, Ordering::Release);
  }

  /// Registers the user guardian PID.
  pub(crate) fn set_user_guardian(&mut self, cell: &ArcShared<ActorCell>) {
    self.guardians.register(GuardianKind::User, cell.pid());
    self.user_guardian_alive.store(true, Ordering::Release);
  }

  /// Returns the guardian kind matching the provided pid when registered.
  pub(crate) fn guardian_kind_by_pid(&self, pid: Pid) -> Option<GuardianKind> {
    self.guardians.kind_by_pid(pid)
  }

  /// Marks the specified guardian as stopped.
  pub(crate) fn mark_guardian_stopped(&self, kind: GuardianKind) {
    self.guardian_alive_flag(kind).store(false, Ordering::Release);
  }

  /// Returns the root guardian cell if initialised.
  #[must_use]
  #[allow(dead_code)]
  pub(crate) fn root_guardian(&self) -> Option<ArcShared<ActorCell>> {
    self.guardian_cell_via_cells(GuardianKind::Root)
  }

  /// Returns the system guardian cell if initialised.
  #[must_use]
  pub(crate) fn system_guardian(&self) -> Option<ArcShared<ActorCell>> {
    self.guardian_cell_via_cells(GuardianKind::System)
  }

  /// Returns the user guardian cell if initialised.
  #[must_use]
  pub(crate) fn user_guardian(&self) -> Option<ArcShared<ActorCell>> {
    self.guardian_cell_via_cells(GuardianKind::User)
  }

  /// Returns the pid of the root guardian if available.
  #[must_use]
  pub const fn root_guardian_pid(&self) -> Option<Pid> {
    self.guardians.pid(GuardianKind::Root)
  }

  /// Returns the pid of the system guardian if available.
  #[must_use]
  pub const fn system_guardian_pid(&self) -> Option<Pid> {
    self.guardians.pid(GuardianKind::System)
  }

  /// Returns the pid of the user guardian if available.
  #[must_use]
  pub const fn user_guardian_pid(&self) -> Option<Pid> {
    self.guardians.pid(GuardianKind::User)
  }

  /// Returns whether the specified guardian is alive.
  #[must_use]
  pub fn guardian_alive(&self, kind: GuardianKind) -> bool {
    self.guardian_alive_flag(kind).load(Ordering::Acquire)
  }

  /// Returns the PID registered for the specified guardian.
  pub const fn guardian_pid(&self, kind: GuardianKind) -> Option<Pid> {
    self.guardians.pid(kind)
  }

  fn guardian_cell_via_cells(&self, kind: GuardianKind) -> Option<ArcShared<ActorCell>> {
    let pid = self.guardians.pid(kind)?;
    self.cell(&pid)
  }

  const fn guardian_alive_flag(&self, kind: GuardianKind) -> &AtomicBool {
    match kind {
      | GuardianKind::Root => &self.root_guardian_alive,
      | GuardianKind::System => &self.system_guardian_alive,
      | GuardianKind::User => &self.user_guardian_alive,
    }
  }

  /// Registers an extra top-level path prior to root startup.
  pub(crate) fn register_extra_top_level(
    &mut self,
    name: &str,
    actor: ActorRef,
  ) -> Result<(), RegisterExtraTopLevelError> {
    if self.root_started.load(Ordering::Acquire) {
      return Err(RegisterExtraTopLevelError::AlreadyStarted);
    }
    if name.is_empty() || RESERVED_TOP_LEVEL.iter().any(|reserved| reserved.eq_ignore_ascii_case(name)) {
      return Err(RegisterExtraTopLevelError::ReservedName(name.into()));
    }
    if self.extra_top_levels.contains_key(name) {
      return Err(RegisterExtraTopLevelError::DuplicateName(name.into()));
    }
    self.extra_top_levels.insert(name.into(), actor);
    Ok(())
  }

  /// Returns a registered extra top-level reference if present.
  #[must_use]
  pub fn extra_top_level(&self, name: &str) -> Option<ActorRef> {
    self.extra_top_levels.get(name)
  }

  /// Marks the root guardian as fully initialised, preventing further registrations.
  pub(crate) fn mark_root_started(&self) {
    self.root_started.store(true, Ordering::Release);
  }

  /// Indicates whether the root guardian has completed startup.
  #[must_use]
  pub fn has_root_started(&self) -> bool {
    self.root_started.load(Ordering::Acquire)
  }

  /// Attempts to transition the system into the terminating state.
  ///
  /// Returns `true` if this call initiated termination, `false` if another caller has already done
  /// so.
  pub fn begin_termination(&self) -> bool {
    self.termination_state.begin_termination()
  }

  /// Indicates whether the system is currently terminating.
  #[must_use]
  pub fn is_terminating(&self) -> bool {
    self.termination_state.is_terminating()
  }

  #[must_use]
  pub(crate) fn next_temp_actor_name(&self) -> String {
    let id = self.temp_counter.fetch_add(1, Ordering::Relaxed) + 1;
    format!("t{:x}", id)
  }

  #[must_use]
  pub(crate) fn next_temp_actor_name_with_prefix(&self, prefix: &str) -> String {
    let id = self.temp_counter.fetch_add(1, Ordering::Relaxed) + 1;
    format!("{prefix}-t{:x}", id)
  }

  /// Resolves the actor path for the specified pid if the actor exists.
  #[must_use]
  pub fn actor_path(&self, pid: &Pid) -> Option<ActorPath> {
    let Some(cell) = self.cell(pid) else {
      let canonical = self.actor_path_registry.canonical_uri(pid)?.to_owned();
      return ActorPathParser::parse(&canonical).ok();
    };
    let mut segments = Vec::new();
    let mut current = Some(cell);
    while let Some(cursor) = current {
      segments.push(cursor.name().to_owned());
      current = cursor.parent().and_then(|parent_pid| self.cell(&parent_pid));
    }
    let identity = self.identity_snapshot();
    if segments.is_empty() {
      return Some(ActorPath::root_with_guardian(identity.guardian_kind));
    }
    segments.pop(); // ルート要素を捨てる
    if segments.is_empty() {
      return Some(ActorPath::root_with_guardian(identity.guardian_kind));
    }
    segments.reverse();
    let (guardian_kind, actor_segments) = match segments.first().map(String::as_str) {
      | Some("system") => (PathGuardianKind::System, &segments[1..]),
      | Some("user") => (PathGuardianKind::User, &segments[1..]),
      | _ => (identity.guardian_kind, segments.as_slice()),
    };
    let mut path = ActorPath::root_with_guardian(guardian_kind);
    for segment in actor_segments {
      path = path.child(segment);
    }
    Some(path)
  }

  /// Returns the shared event stream handle.
  #[must_use]
  pub fn event_stream(&self) -> EventStreamShared {
    self.event_stream.clone()
  }

  /// Returns a snapshot of deadletter entries.
  #[must_use]
  pub fn dead_letters(&self) -> Vec<DeadLetterEntry> {
    self.dead_letter.entries()
  }

  /// Returns the shared deadletter store.
  #[must_use]
  pub(crate) fn dead_letter_store(&self) -> DeadLetterShared {
    self.dead_letter.clone()
  }

  /// Returns `true` when an extension for the provided [`TypeId`] is registered.
  #[must_use]
  pub(crate) fn has_extension(&self, type_id: TypeId) -> bool {
    self.extensions.contains_key(&type_id)
  }

  /// Returns an extension by [`TypeId`].
  pub(crate) fn extension<E>(&self, type_id: TypeId) -> Option<ArcShared<E>>
  where
    E: Any + Send + Sync + 'static, {
    self.extensions.get(&type_id).cloned().and_then(|handle| handle.downcast::<E>().ok())
  }

  /// Returns a raw extension handle by [`TypeId`].
  pub(crate) fn extension_raw(&self, type_id: &TypeId) -> Option<ArcShared<dyn Any + Send + Sync + 'static>> {
    self.extensions.get(type_id).cloned()
  }

  /// Inserts an extension.
  pub(crate) fn insert_extension(&mut self, type_id: TypeId, extension: ArcShared<dyn Any + Send + Sync + 'static>) {
    self.extensions.insert(type_id, extension);
  }

  pub(crate) fn extension_by_type<E>(&self) -> Option<ArcShared<E>>
  where
    E: Any + Send + Sync + 'static, {
    for handle in self.extensions.values() {
      if let Ok(extension) = handle.clone().downcast::<E>() {
        return Some(extension);
      }
    }
    None
  }

  /// Publishes an event to all event stream subscribers.
  pub fn publish_event(&self, event: &EventStreamEvent) {
    self.event_stream.publish(event);
  }

  /// Emits a log event via the event stream.
  pub fn emit_log(&self, level: LogLevel, message: String, origin: Option<Pid>, logger_name: Option<String>) {
    let timestamp = self.monotonic_now();
    let event = LogEvent::new(level, message, timestamp, origin, logger_name);
    self.event_stream.publish(&EventStreamEvent::Log(event));
  }

  pub(crate) fn install_actor_ref_provider<P>(&mut self, provider: &ActorRefProviderHandleShared<P>)
  where
    P: ActorRefProvider + Any + Send + Sync + 'static, {
    let erased: ArcShared<dyn Any + Send + Sync + 'static> = ArcShared::new(provider.clone());
    self.actor_ref_providers.insert(TypeId::of::<P>(), erased);
    let schemes = provider.supported_schemes().to_vec();
    for scheme in schemes {
      let cloned = provider.clone();
      let caller: ActorRefProviderCaller = ArcShared::new(move |path| cloned.get_actor_ref(path));
      self.actor_ref_provider_callers_by_scheme.insert(scheme, caller);
    }
  }

  pub(crate) fn actor_ref_provider<P>(&self) -> Option<ActorRefProviderHandleShared<P>>
  where
    P: ActorRefProvider + Any + Send + Sync + 'static, {
    self
      .actor_ref_providers
      .get(&TypeId::of::<P>())
      .cloned()
      .and_then(|provider| provider.downcast::<ActorRefProviderHandleShared<P>>().ok())
      .map(|provider| (*provider).clone())
  }

  pub(crate) fn actor_ref_provider_caller_for_scheme(&self, scheme: ActorPathScheme) -> Option<ActorRefProviderCaller> {
    self.actor_ref_provider_callers_by_scheme.get(scheme).cloned()
  }

  fn forward_remote_watch(&self, target: Pid, watcher: Pid) -> bool {
    self.remote_watch_hook.handle_watch(target, watcher)
  }

  fn forward_remote_unwatch(&self, target: Pid, watcher: Pid) -> bool {
    self.remote_watch_hook.handle_unwatch(target, watcher)
  }

  /// Sends a system message to the specified actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the actor doesn't exist or the message cannot be enqueued.
  pub(crate) fn send_system_message(&self, pid: Pid, message: SystemMessage) -> Result<(), SendError> {
    if let Some(cell) = self.cell(&pid) {
      cell.new_dispatcher_shared().system_dispatch(&cell, message)
    } else {
      match message {
        | SystemMessage::Watch(watcher) => {
          if self.forward_remote_watch(pid, watcher) {
            return Ok(());
          }
          if let Err(e) = self.send_system_message(watcher, SystemMessage::Terminated(pid)) {
            self.record_send_error(Some(watcher), &e);
          }
          Ok(())
        },
        | SystemMessage::Unwatch(watcher) => {
          if self.forward_remote_unwatch(pid, watcher) {
            return Ok(());
          }
          Ok(())
        },
        | SystemMessage::Terminated(_) => Ok(()),
        | SystemMessage::PipeTask(_) => Ok(()),
        | other => Err(SendError::closed(AnyMessage::new(other))),
      }
    }
  }

  /// Records a send error for diagnostics.
  pub(crate) fn record_send_error(&self, recipient: Option<Pid>, error: &SendError) {
    let timestamp = self.monotonic_now();
    self.dead_letter.record_send_error(recipient, error, timestamp);
  }

  /// Marks the system as terminated and wakes all observers.
  pub(crate) fn mark_terminated(&self) {
    self.termination_state.mark_terminated();
  }

  /// Returns a shared reference to the termination state.
  #[must_use]
  pub(crate) fn termination_state(&self) -> ArcShared<TerminationState> {
    self.termination_state.clone()
  }

  /// Indicates whether the actor system has terminated.
  #[must_use]
  pub fn is_terminated(&self) -> bool {
    self.termination_state.is_terminated()
  }

  /// Returns a monotonic timestamp for instrumentation.
  #[must_use]
  pub fn monotonic_now(&self) -> Duration {
    let ticks = self.clock.fetch_add(1, Ordering::Relaxed) + 1;
    Duration::from_millis(ticks)
  }

  /// Resolves a [`MessageDispatcherShared`] for the identifier.
  ///
  /// Returns `None` when no configurator is registered under that identifier.
  #[must_use]
  pub fn resolve_dispatcher(&self, id: &str) -> Option<MessageDispatcherShared> {
    self.dispatchers.resolve(id).ok()
  }

  /// Returns the cumulative number of `Dispatchers::resolve` invocations
  /// observed by the actor system's dispatcher registry.
  ///
  /// Diagnostics-only accessor used by integration tests to verify the
  /// `Dispatchers::resolve` call-frequency contract: message hot paths must
  /// not bump the counter once the system has finished bootstrapping.
  #[must_use]
  pub fn dispatcher_resolve_call_count(&self) -> usize {
    self.dispatchers.resolve_call_count()
  }

  /// Resolves the mailbox configuration for the identifier.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Unknown`] when the identifier has not been registered.
  pub fn resolve_mailbox(&self, id: &str) -> Result<MailboxConfig, MailboxRegistryError> {
    self.mailboxes.resolve(id)
  }

  /// Creates a mailbox queue from the configuration registered under the identifier.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Unknown`] when the identifier has not been registered.
  pub fn create_mailbox_queue(&self, id: &str) -> Result<Box<dyn MessageQueue>, MailboxRegistryError> {
    self.mailboxes.create_message_queue(id)
  }

  /// Returns the remoting configuration when it has been configured.
  #[must_use]
  pub fn remoting_config(&self) -> Option<RemotingConfig> {
    let identity = self.identity_snapshot();
    identity.canonical_host.map(|host| {
      let mut config =
        RemotingConfig::default().with_canonical_host(host).with_quarantine_duration(identity.quarantine_duration);
      if let Some(port) = identity.canonical_port {
        config = config.with_canonical_port(port);
      }
      config
    })
  }

  /// Returns the shared scheduler handle.
  #[must_use]
  pub fn scheduler(&self) -> SchedulerShared {
    self.scheduler_context.scheduler()
  }

  /// Returns the delay provider connected to the scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider {
    self.scheduler_context.delay_provider()
  }

  /// Returns the tick driver bundle.
  #[must_use]
  pub fn tick_driver_bundle(&self) -> TickDriverBundle {
    self.tick_driver_bundle.clone()
  }

  /// Returns the last recorded tick driver snapshot when available.
  #[must_use]
  pub fn tick_driver_snapshot(&self) -> Option<TickDriverSnapshot> {
    self.tick_driver_snapshot.clone()
  }

  /// Shuts down the scheduler context if configured.
  pub fn shutdown_scheduler(&self) -> Option<TaskRunSummary> {
    let scheduler = self.scheduler();
    Some(scheduler.with_write(|s| s.shutdown_with_tasks()))
  }

  pub(crate) fn record_failure_reported(&self) {
    self.failure_total.fetch_add(1, Ordering::Relaxed);
    self.failure_inflight.fetch_add(1, Ordering::AcqRel);
  }

  /// Records the outcome of a previously reported failure (restart/stop/escalate).
  pub(crate) fn record_failure_outcome(&self, child: Pid, outcome: FailureOutcome, payload: &FailurePayload) {
    self.failure_inflight.fetch_sub(1, Ordering::AcqRel);
    let counter = match outcome {
      | FailureOutcome::Restart => &self.failure_restart_total,
      | FailureOutcome::Stop => &self.failure_stop_total,
      | FailureOutcome::Escalate => &self.failure_escalate_total,
      | FailureOutcome::Resume => &self.failure_resume_total,
    };
    counter.fetch_add(1, Ordering::Relaxed);
    let label = match outcome {
      | FailureOutcome::Restart => "restart",
      | FailureOutcome::Stop => "stop",
      | FailureOutcome::Escalate => "escalate",
      | FailureOutcome::Resume => "resume",
    };
    let message = format!("failure outcome {} for {:?} (reason: {})", label, child, payload.reason().as_str());
    self.emit_log(LogLevel::Info, message, Some(child), None);
  }

  #[allow(dead_code)]
  pub(crate) fn handle_failure(&self, pid: Pid, parent: Option<Pid>, error: &ActorError) {
    let Some(parent_pid) = parent else {
      self.stop_actor(pid);
      return;
    };

    let Some(parent_cell) = self.cell(&parent_pid) else {
      self.stop_actor(pid);
      return;
    };

    let parent_cell_ref = &*parent_cell;
    let parent_parent = parent_cell_ref.parent();
    let now = self.monotonic_now();
    let (directive, affected) = parent_cell_ref.handle_child_failure(pid, error, now);

    match directive {
      | SupervisorDirective::Restart => {
        let mut escalate_due_to_recreate_failure = false;
        for target in affected {
          if let Err(send_error) = self.send_system_message(target, SystemMessage::Recreate) {
            self.record_send_error(Some(target), &send_error);
            if let Err(e) = self.send_system_message(target, SystemMessage::Stop) {
              self.record_send_error(Some(target), &e);
            }
            escalate_due_to_recreate_failure = true;
          }
        }
        if escalate_due_to_recreate_failure {
          self.handle_failure(parent_pid, parent_parent, error);
        }
      },
      | SupervisorDirective::Stop => {
        for target in affected {
          self.stop_actor(target);
        }
      },
      | SupervisorDirective::Escalate => {
        for target in affected {
          self.stop_actor(target);
        }
        self.handle_failure(parent_pid, parent_parent, error);
      },
      | SupervisorDirective::Resume => {
        for target in affected {
          if let Err(e) = self.send_system_message(target, SystemMessage::Resume) {
            self.record_send_error(Some(target), &e);
          }
        }
      },
    }
  }

  fn stop_actor(&self, pid: Pid) {
    if let Err(e) = self.send_system_message(pid, SystemMessage::Stop) {
      self.record_send_error(Some(pid), &e);
    }
  }

  /// Returns a reference to the ActorPathRegistry.
  #[must_use]
  pub const fn actor_path_registry(&self) -> &ActorPathRegistry {
    &self.actor_path_registry
  }

  pub(crate) const fn actor_path_registry_mut(&mut self) -> &mut ActorPathRegistry {
    &mut self.actor_path_registry
  }

  /// Returns the current authority state.
  #[must_use]
  pub fn remote_authority_state(&self, authority: &str) -> AuthorityState {
    self.remote_authority_registry.state(authority)
  }

  /// Returns a snapshot of known remote authorities and their states.
  pub fn remote_authority_snapshots(&self) -> Vec<(String, AuthorityState)> {
    self.remote_authority_registry.snapshots()
  }

  /// Marks the authority as connected and emits an event.
  pub fn remote_authority_set_connected(&mut self, authority: &str) -> Option<VecDeque<AnyMessage>> {
    let drained = self.remote_authority_registry.set_connected(authority);
    self.publish_remote_authority_event(authority.to_string(), AuthorityState::Connected);
    drained
  }

  /// Transitions the authority into quarantine using the provided duration or the configured
  /// default.
  pub fn remote_authority_set_quarantine(&mut self, authority: impl Into<String>, duration: Option<Duration>) {
    let authority = authority.into();
    let now_secs = self.monotonic_now().as_secs();
    let effective = duration.unwrap_or(self.default_quarantine_duration());
    self.remote_authority_registry.set_quarantine(authority.clone(), now_secs, Some(effective));
    let state = self.remote_authority_registry.state(&authority);
    self.publish_remote_authority_event(authority, state);
  }

  /// Handles an InvalidAssociation signal by moving the authority into quarantine.
  pub fn remote_authority_handle_invalid_association(
    &mut self,
    authority: impl Into<String>,
    duration: Option<Duration>,
  ) {
    let authority = authority.into();
    let now_secs = self.monotonic_now().as_secs();
    let effective = duration.unwrap_or(self.default_quarantine_duration());
    self.remote_authority_registry.handle_invalid_association(authority.clone(), now_secs, Some(effective));
    let state = self.remote_authority_registry.state(&authority);
    self.publish_remote_authority_event(authority, state);
  }

  /// Manually overrides a quarantined authority back to connected.
  pub fn remote_authority_manual_override_to_connected(&mut self, authority: &str) {
    self.remote_authority_registry.manual_override_to_connected(authority);
    self.publish_remote_authority_event(authority.to_string(), AuthorityState::Connected);
  }

  /// Defers a message while the authority is unresolved.
  ///
  /// # Errors
  ///
  /// Returns [`RemoteAuthorityError::Quarantined`] if the target authority is currently
  /// quarantined.
  pub fn remote_authority_defer(
    &mut self,
    authority: impl Into<String>,
    message: AnyMessage,
  ) -> Result<(), RemoteAuthorityError> {
    self.remote_authority_registry.defer_send(authority, message)
  }

  /// Attempts to defer a message, returning an error if the authority is quarantined.
  ///
  /// # Errors
  ///
  /// Returns [`RemoteAuthorityError::Quarantined`] when the authority remains quarantined.
  pub fn remote_authority_try_defer(
    &mut self,
    authority: impl Into<String>,
    message: AnyMessage,
  ) -> Result<(), RemoteAuthorityError> {
    self.remote_authority_registry.try_defer_send(authority, message)
  }

  /// Polls all authorities for expired quarantine windows and emits events for lifted entries.
  pub fn poll_remote_authorities(&mut self) {
    let now_secs = self.monotonic_now().as_secs();
    self.actor_path_registry.poll_expired(now_secs);
    let lifted = self.remote_authority_registry.poll_quarantine_expiration(now_secs);
    for authority in lifted {
      self.publish_remote_authority_event(authority.clone(), AuthorityState::Unresolved);
    }
  }

  /// Returns the number of messages deferred for the provided authority.
  #[must_use]
  pub fn remote_authority_deferred_count(&self, authority: &str) -> usize {
    self.remote_authority_registry.deferred_count(authority)
  }
}

impl Drop for SystemState {
  fn drop(&mut self) {
    self.tick_driver_bundle.shutdown();
  }
}

impl Default for SystemState {
  fn default() -> Self {
    Self::new()
  }
}
