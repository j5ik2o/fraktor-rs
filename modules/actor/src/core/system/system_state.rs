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

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess},
};
use portable_atomic::{AtomicBool, AtomicU64, Ordering};

use self::path_identity::PathIdentity;
use super::{
  ActorPathRegistrySharedGeneric, ActorRefProvider, ActorRefProviderCaller, ActorRefProviderHandle,
  ActorRefProviderSharedGeneric, AskFuturesSharedGeneric, AuthorityState, CellsSharedGeneric, GuardianKind,
  RegistriesSharedGeneric, RemoteAuthorityError, RemoteAuthorityManagerSharedGeneric, RemoteWatchHookDynSharedGeneric,
  RemotingConfig, TempActorsSharedGeneric, actor_ref_provider_callers::ActorRefProviderCallersGeneric,
  actor_ref_providers::ActorRefProvidersGeneric, extensions::ExtensionsGeneric,
  extra_top_levels::ExtraTopLevelsGeneric, guardians_state::GuardiansState,
};
use crate::core::{
  actor_prim::{
    ActorCellGeneric, Pid,
    actor_path::{ActorPath, ActorPathScheme, GuardianKind as PathGuardianKind},
    actor_ref::ActorRefGeneric,
  },
  dead_letter::{DeadLetterEntryGeneric, DeadLetterSharedGeneric},
  dispatcher::DispatchersGeneric,
  error::{ActorError, SendError},
  event_stream::{EventStreamEvent, EventStreamSharedGeneric, RemoteAuthorityEvent, TickDriverSnapshot},
  futures::ActorFutureSharedGeneric,
  logging::{LogEvent, LogLevel},
  mailbox::MailboxesGeneric,
  messaging::{AnyMessageGeneric, FailurePayload, SystemMessage},
  scheduler::{
    SchedulerConfig, SchedulerContextSharedGeneric, TaskRunSummary, TickDriverControl, TickDriverHandleGeneric,
    TickDriverKind, TickDriverRuntime, TickExecutorSignal, TickFeed, next_tick_driver_id,
  },
  spawn::SpawnError,
  supervision::SupervisorDirective,
  system::{RegisterExtraTopLevelError, ReservationPolicy},
};

mod failure_outcome;

pub use failure_outcome::FailureOutcome;

use crate::core::system::actor_system_config::ActorSystemConfigGeneric;

const RESERVED_TOP_LEVEL: [&str; 4] = ["user", "system", "temp", "deadLetters"];

/// Captures global actor system state.
pub struct SystemStateGeneric<TB: RuntimeToolbox + 'static> {
  next_pid: AtomicU64,
  clock: AtomicU64,
  cells: CellsSharedGeneric<TB>,
  registries: RegistriesSharedGeneric<TB>,
  guardians: GuardiansState,
  root_guardian_alive: AtomicBool,
  system_guardian_alive: AtomicBool,
  user_guardian_alive: AtomicBool,
  ask_futures: AskFuturesSharedGeneric<TB>,
  termination: ActorFutureSharedGeneric<(), TB>,
  terminated: AtomicBool,
  terminating: AtomicBool,
  root_started: AtomicBool,
  event_stream: EventStreamSharedGeneric<TB>,
  dead_letter: DeadLetterSharedGeneric<TB>,
  extra_top_levels: ExtraTopLevelsGeneric<TB>,
  temp_actors: TempActorsSharedGeneric<TB>,
  temp_counter: AtomicU64,
  failure_total: AtomicU64,
  failure_restart_total: AtomicU64,
  failure_stop_total: AtomicU64,
  failure_escalate_total: AtomicU64,
  failure_inflight: AtomicU64,
  extensions: ExtensionsGeneric<TB>,
  actor_ref_providers: ActorRefProvidersGeneric<TB>,
  actor_ref_provider_callers_by_scheme: ActorRefProviderCallersGeneric<TB>,
  remote_watch_hook: RemoteWatchHookDynSharedGeneric<TB>,
  dispatchers: ArcShared<DispatchersGeneric<TB>>,
  mailboxes: ArcShared<MailboxesGeneric<TB>>,
  path_identity: PathIdentity,
  actor_path_registry: ActorPathRegistrySharedGeneric<TB>,
  remote_authority_mgr: RemoteAuthorityManagerSharedGeneric<TB>,
  scheduler_context: SchedulerContextSharedGeneric<TB>,
  tick_driver_runtime: TickDriverRuntime<TB>,
}

/// Type alias for [SystemStateGeneric] with the default [NoStdToolbox].
pub type SystemState = SystemStateGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> SystemStateGeneric<TB> {
  /// Creates a fresh state container without any registered actors.
  #[must_use]
  pub fn new() -> Self
  where
    TB: Default, {
    const DEAD_LETTER_CAPACITY: usize = 512;
    let event_stream = EventStreamSharedGeneric::default();
    let dead_letter = DeadLetterSharedGeneric::with_capacity(event_stream.clone(), DEAD_LETTER_CAPACITY);
    let mut dispatchers = DispatchersGeneric::new();
    dispatchers.ensure_default();
    let dispatchers = ArcShared::new(dispatchers);
    let mut mailboxes = MailboxesGeneric::<TB>::new();
    mailboxes.ensure_default();
    let scheduler_config = SchedulerConfig::default();
    let toolbox = TB::default();
    let scheduler_context =
      SchedulerContextSharedGeneric::with_event_stream(toolbox, scheduler_config, event_stream.clone());
    let tick_driver_runtime = Self::default_tick_driver_runtime(scheduler_config.resolution());
    Self {
      next_pid: AtomicU64::new(0),
      clock: AtomicU64::new(0),
      cells: CellsSharedGeneric::default(),
      registries: RegistriesSharedGeneric::default(),
      guardians: GuardiansState::default(),
      root_guardian_alive: AtomicBool::new(false),
      system_guardian_alive: AtomicBool::new(false),
      user_guardian_alive: AtomicBool::new(false),
      ask_futures: AskFuturesSharedGeneric::default(),
      termination: ActorFutureSharedGeneric::<(), TB>::new(),
      terminated: AtomicBool::new(false),
      terminating: AtomicBool::new(false),
      root_started: AtomicBool::new(false),
      event_stream,
      dead_letter,
      extra_top_levels: ExtraTopLevelsGeneric::default(),
      temp_actors: TempActorsSharedGeneric::default(),
      temp_counter: AtomicU64::new(0),
      failure_total: AtomicU64::new(0),
      failure_restart_total: AtomicU64::new(0),
      failure_stop_total: AtomicU64::new(0),
      failure_escalate_total: AtomicU64::new(0),
      failure_inflight: AtomicU64::new(0),
      extensions: ExtensionsGeneric::default(),
      actor_ref_providers: ActorRefProvidersGeneric::default(),
      remote_watch_hook: RemoteWatchHookDynSharedGeneric::noop(),
      dispatchers,
      mailboxes: ArcShared::new(mailboxes),
      path_identity: PathIdentity::default(),
      actor_path_registry: ActorPathRegistrySharedGeneric::default(),
      remote_authority_mgr: RemoteAuthorityManagerSharedGeneric::default(),
      actor_ref_provider_callers_by_scheme: ActorRefProviderCallersGeneric::default(),
      scheduler_context,
      tick_driver_runtime,
    }
  }

  pub(crate) fn build_from_config(config: &ActorSystemConfigGeneric<TB>) -> Result<Self, SpawnError>
  where
    TB: Default, {
    use crate::core::scheduler::TickDriverBootstrap;

    let mut state = Self::new();
    state.apply_actor_system_config(config);

    let event_stream = state.event_stream();
    let toolbox = TB::default();
    let scheduler_config = *config.scheduler_config();
    #[cfg(any(test, feature = "test-support"))]
    let scheduler_config = if let Some(tick_driver_config) = config.tick_driver_config()
      && matches!(tick_driver_config, crate::core::scheduler::TickDriverConfig::ManualTest(_))
      && !scheduler_config.runner_api_enabled()
    {
      scheduler_config.with_runner_api_enabled(true)
    } else {
      scheduler_config
    };

    let context = SchedulerContextSharedGeneric::with_event_stream(toolbox, scheduler_config, event_stream);
    state.scheduler_context = context.clone();

    let tick_driver_config = config
      .tick_driver_config()
      .ok_or_else(|| SpawnError::SystemBuildError("tick driver configuration is required".into()))?;
    let runtime = TickDriverBootstrap::provision(tick_driver_config, &context)
      .map_err(|error| SpawnError::SystemBuildError(format!("tick driver provisioning failed: {error}")))?;
    state.tick_driver_runtime = runtime;

    Ok(state)
  }

  fn default_tick_driver_runtime(resolution: Duration) -> TickDriverRuntime<TB> {
    struct NoopDriverControl;

    impl TickDriverControl for NoopDriverControl {
      fn shutdown(&self) {}
    }

    let signal = TickExecutorSignal::new();
    let feed = TickFeed::new(resolution, 1, signal);
    let control: Box<dyn TickDriverControl> = Box::new(NoopDriverControl);
    let control = ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(control));
    let handle = TickDriverHandleGeneric::new(next_tick_driver_id(), TickDriverKind::Auto, resolution, control);
    TickDriverRuntime::new(handle, feed)
  }

  /// Allocates a new unique [`Pid`] for an actor.
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    let value = self.next_pid.fetch_add(1, Ordering::Relaxed) + 1;
    Pid::new(value, 0)
  }

  /// Applies the actor system configuration (system name, remoting settings).
  pub fn apply_actor_system_config(&mut self, config: &ActorSystemConfigGeneric<TB>) {
    self.path_identity.system_name = config.system_name().to_string();
    self.path_identity.guardian_kind = config.default_guardian();
    self.dispatchers = ArcShared::new(config.dispatchers().clone());
    self.mailboxes = ArcShared::new(config.mailboxes().clone());
    if let Some(remoting) = config.remoting_config() {
      self.path_identity.canonical_host = Some(remoting.canonical_host().to_string());
      self.path_identity.canonical_port = remoting.canonical_port();
      self.path_identity.quarantine_duration = remoting.quarantine_duration();
    } else {
      self.path_identity.canonical_host = None;
      self.path_identity.canonical_port = None;
      self.path_identity.quarantine_duration = path_identity::DEFAULT_QUARANTINE_DURATION;
    }

    let policy = ReservationPolicy::with_quarantine_duration(self.default_quarantine_duration());
    self.actor_path_registry.with_write(|registry| registry.set_policy(policy));
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
  pub(crate) fn cell(&self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.cells.with_read(|cells| cells.get(pid))
  }

  /// Returns the shared cell registry handle.
  #[must_use]
  pub(crate) fn cells_handle(&self) -> CellsSharedGeneric<TB> {
    self.cells.clone()
  }

  /// Returns the shared name registries handle.
  #[must_use]
  pub(crate) fn registries_handle(&self) -> RegistriesSharedGeneric<TB> {
    self.registries.clone()
  }

  /// Returns the shared ask futures handle.
  #[must_use]
  pub(crate) fn ask_futures_handle(&self) -> AskFuturesSharedGeneric<TB> {
    self.ask_futures.clone()
  }

  /// Returns the shared temporary actor registry handle.
  #[must_use]
  pub(crate) fn temp_actors_handle(&self) -> TempActorsSharedGeneric<TB> {
    self.temp_actors.clone()
  }

  /// Returns the shared remote watch hook handle.
  #[must_use]
  pub(crate) fn remote_watch_hook_handle(&self) -> RemoteWatchHookDynSharedGeneric<TB> {
    self.remote_watch_hook.clone()
  }

  /// Registers the root guardian PID.
  pub(crate) fn set_root_guardian(&mut self, cell: &ArcShared<ActorCellGeneric<TB>>) {
    self.guardians.register(GuardianKind::Root, cell.pid());
    self.root_guardian_alive.store(true, Ordering::Release);
  }

  #[cfg(any(test, feature = "test-support"))]
  pub(crate) fn register_guardian_pid(&mut self, kind: GuardianKind, pid: Pid) {
    self.guardians.register(kind, pid);
    self.guardian_alive_flag(kind).store(true, Ordering::Release);
  }

  /// Registers the system guardian PID.
  pub(crate) fn set_system_guardian(&mut self, cell: &ArcShared<ActorCellGeneric<TB>>) {
    self.guardians.register(GuardianKind::System, cell.pid());
    self.system_guardian_alive.store(true, Ordering::Release);
  }

  /// Registers the user guardian PID.
  pub(crate) fn set_user_guardian(&mut self, cell: &ArcShared<ActorCellGeneric<TB>>) {
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
  pub(crate) fn root_guardian(&self) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.guardian_cell_via_cells(GuardianKind::Root)
  }

  /// Returns the system guardian cell if initialised.
  #[must_use]
  pub(crate) fn system_guardian(&self) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.guardian_cell_via_cells(GuardianKind::System)
  }

  /// Returns the user guardian cell if initialised.
  #[must_use]
  pub(crate) fn user_guardian(&self) -> Option<ArcShared<ActorCellGeneric<TB>>> {
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

  fn guardian_cell_via_cells(&self, kind: GuardianKind) -> Option<ArcShared<ActorCellGeneric<TB>>> {
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
    actor: ActorRefGeneric<TB>,
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
  pub fn extra_top_level(&self, name: &str) -> Option<ActorRefGeneric<TB>> {
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
    !self.terminating.swap(true, Ordering::AcqRel)
  }

  /// Indicates whether the system is currently terminating.
  #[must_use]
  pub fn is_terminating(&self) -> bool {
    self.terminating.load(Ordering::Acquire)
  }

  #[must_use]
  pub(crate) fn next_temp_actor_name(&self) -> String {
    let id = self.temp_counter.fetch_add(1, Ordering::Relaxed) + 1;
    format!("t{:x}", id)
  }

  /// Resolves the actor path for the specified pid if the actor exists.
  #[must_use]
  pub fn actor_path(&self, pid: &Pid) -> Option<ActorPath> {
    let cell = self.cell(pid)?;
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
    let mut path = ActorPath::root_with_guardian(identity.guardian_kind);
    for segment in segments {
      path = path.child(segment);
    }
    Some(path)
  }

  /// Returns the shared event stream handle.
  #[must_use]
  pub fn event_stream(&self) -> EventStreamSharedGeneric<TB> {
    self.event_stream.clone()
  }

  /// Returns a snapshot of deadletter entries.
  #[must_use]
  pub fn dead_letters(&self) -> Vec<DeadLetterEntryGeneric<TB>> {
    self.dead_letter.entries()
  }

  /// Returns the shared deadletter store.
  #[must_use]
  pub(crate) fn dead_letter_store(&self) -> DeadLetterSharedGeneric<TB> {
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
  pub fn publish_event(&self, event: &EventStreamEvent<TB>) {
    self.event_stream.publish(event);
  }

  /// Emits a log event via the event stream.
  pub fn emit_log(&self, level: LogLevel, message: String, origin: Option<Pid>) {
    let timestamp = self.monotonic_now();
    let event = LogEvent::new(level, message, timestamp, origin);
    self.event_stream.publish(&EventStreamEvent::Log(event));
  }

  pub(crate) fn install_actor_ref_provider<P>(&mut self, provider: &ActorRefProviderSharedGeneric<TB, P>)
  where
    P: ActorRefProvider<TB> + Any + Send + Sync + 'static, {
    let erased: ArcShared<dyn Any + Send + Sync + 'static> = provider.inner().clone();
    self.actor_ref_providers.insert(TypeId::of::<P>(), erased);
    let schemes = provider.supported_schemes().to_vec();
    for scheme in schemes {
      let cloned = provider.clone();
      let caller: ActorRefProviderCaller<TB> = ArcShared::new(move |path| cloned.get_actor_ref(path));
      self.actor_ref_provider_callers_by_scheme.insert(scheme, caller);
    }
  }

  pub(crate) fn actor_ref_provider<P>(&self) -> Option<ActorRefProviderSharedGeneric<TB, P>>
  where
    P: ActorRefProvider<TB> + Any + Send + Sync + 'static, {
    self
      .actor_ref_providers
      .get(&TypeId::of::<P>())
      .cloned()
      .and_then(|provider| provider.downcast::<ToolboxMutex<ActorRefProviderHandle<P>, TB>>().ok())
      .map(ActorRefProviderSharedGeneric::from_shared)
  }

  pub(crate) fn actor_ref_provider_caller_for_scheme(
    &self,
    scheme: ActorPathScheme,
  ) -> Option<ActorRefProviderCaller<TB>> {
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
  pub(crate) fn send_system_message(&self, pid: Pid, message: SystemMessage) -> Result<(), SendError<TB>> {
    if let Some(cell) = self.cell(&pid) {
      cell.dispatcher().enqueue_system(message)
    } else {
      match message {
        | SystemMessage::Watch(watcher) => {
          if self.forward_remote_watch(pid, watcher) {
            return Ok(());
          }
          let _ = self.send_system_message(watcher, SystemMessage::Terminated(pid));
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
        | other => Err(SendError::<TB>::closed(AnyMessageGeneric::new(other))),
      }
    }
  }

  /// Records a send error for diagnostics.
  pub(crate) fn record_send_error(&self, recipient: Option<Pid>, error: &SendError<TB>) {
    let timestamp = self.monotonic_now();
    self.dead_letter.record_send_error(recipient, error, timestamp);
  }

  /// Marks the system as terminated and completes the termination future.
  pub(crate) fn mark_terminated(&self) {
    self.terminating.store(true, Ordering::Release);
    if self.terminated.swap(true, Ordering::AcqRel) {
      return;
    }
    // ロック中に完了させ、wake はロック外で行ってデッドロックを避ける
    let waker = self.termination.with_write(|af| af.complete(()));
    if let Some(w) = waker {
      w.wake();
    }
  }

  /// Returns a future that resolves once the actor system terminates.
  #[must_use]
  pub(crate) fn termination_future(&self) -> ActorFutureSharedGeneric<(), TB> {
    self.termination.clone()
  }

  /// Indicates whether the actor system has terminated.
  #[must_use]
  pub fn is_terminated(&self) -> bool {
    self.terminated.load(Ordering::Acquire)
  }

  /// Returns a monotonic timestamp for instrumentation.
  #[must_use]
  pub fn monotonic_now(&self) -> Duration {
    let ticks = self.clock.fetch_add(1, Ordering::Relaxed) + 1;
    Duration::from_millis(ticks)
  }

  /// Returns the dispatcher registry.
  #[must_use]
  pub fn dispatchers(&self) -> ArcShared<DispatchersGeneric<TB>> {
    self.dispatchers.clone()
  }

  /// Returns the mailbox registry.
  #[must_use]
  pub fn mailboxes(&self) -> ArcShared<MailboxesGeneric<TB>> {
    self.mailboxes.clone()
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

  /// Returns the scheduler context.
  #[must_use]
  pub fn scheduler_context(&self) -> SchedulerContextSharedGeneric<TB> {
    self.scheduler_context.clone()
  }

  /// Returns the tick driver runtime.
  #[must_use]
  pub fn tick_driver_runtime(&self) -> TickDriverRuntime<TB> {
    self.tick_driver_runtime.clone()
  }

  /// Returns the last recorded tick driver snapshot when available.
  #[must_use]
  pub fn tick_driver_snapshot(&self) -> Option<TickDriverSnapshot> {
    self.scheduler_context().driver_snapshot()
  }

  /// Shuts down the scheduler context if configured.
  pub fn shutdown_scheduler(&self) -> Option<TaskRunSummary> {
    Some(self.scheduler_context().shutdown())
  }

  /// Records a failure and routes it to the supervising hierarchy.
  pub(crate) fn report_failure(&self, mut payload: FailurePayload) {
    self.failure_total.fetch_add(1, Ordering::Relaxed);
    self.failure_inflight.fetch_add(1, Ordering::AcqRel);
    let message = format!("actor {:?} failed: {}", payload.child(), payload.reason().as_str());
    self.emit_log(LogLevel::Error, message, Some(payload.child()));

    if let Some(parent_pid) = self.parent_of(&payload.child())
      && let Some(parent_cell) = self.cell(&parent_pid)
    {
      if let Some(stats) = parent_cell.snapshot_child_restart_stats(payload.child()) {
        payload = payload.with_restart_stats(stats);
      }
      if self.send_system_message(parent_pid, SystemMessage::Failure(payload.clone())).is_ok() {
        return;
      }
      let payload_ref = &payload;
      self.record_failure_outcome(payload.child(), FailureOutcome::Stop, payload_ref);
      self.stop_actor(payload.child());
      return;
    }

    let payload_ref = &payload;
    self.record_failure_outcome(payload.child(), FailureOutcome::Stop, payload_ref);
    self.stop_actor(payload.child());
  }

  /// Records the outcome of a previously reported failure (restart/stop/escalate).
  pub(crate) fn record_failure_outcome(&self, child: Pid, outcome: FailureOutcome, payload: &FailurePayload) {
    self.failure_inflight.fetch_sub(1, Ordering::AcqRel);
    let counter = match outcome {
      | FailureOutcome::Restart => &self.failure_restart_total,
      | FailureOutcome::Stop => &self.failure_stop_total,
      | FailureOutcome::Escalate => &self.failure_escalate_total,
    };
    counter.fetch_add(1, Ordering::Relaxed);
    let label = match outcome {
      | FailureOutcome::Restart => "restart",
      | FailureOutcome::Stop => "stop",
      | FailureOutcome::Escalate => "escalate",
    };
    let message = format!("failure outcome {} for {:?} (reason: {})", label, child, payload.reason().as_str());
    self.emit_log(LogLevel::Info, message, Some(child));
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
            let _ = self.send_system_message(target, SystemMessage::Stop);
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
    }
  }

  fn stop_actor(&self, pid: Pid) {
    let _ = self.send_system_message(pid, SystemMessage::Stop);
  }

  fn parent_of(&self, pid: &Pid) -> Option<Pid> {
    self.cell(pid).and_then(|cell| cell.parent())
  }

  /// Returns a reference to the ActorPathRegistry.
  #[must_use]
  pub const fn actor_path_registry(&self) -> &ActorPathRegistrySharedGeneric<TB> {
    &self.actor_path_registry
  }

  /// Returns a reference to the RemoteAuthorityManager.
  #[must_use]
  pub const fn remote_authority_manager(&self) -> &RemoteAuthorityManagerSharedGeneric<TB> {
    &self.remote_authority_mgr
  }

  /// Returns the current authority state.
  #[must_use]
  pub fn remote_authority_state(&self, authority: &str) -> AuthorityState {
    self.remote_authority_mgr.with_read(|mgr| mgr.state(authority))
  }

  /// Returns a snapshot of known remote authorities and their states.
  pub fn remote_authority_snapshots(&self) -> Vec<(String, AuthorityState)> {
    self.remote_authority_mgr.with_read(|mgr| mgr.snapshots())
  }

  /// Marks the authority as connected and emits an event.
  pub fn remote_authority_set_connected(&self, authority: &str) -> Option<VecDeque<AnyMessageGeneric<TB>>> {
    let drained = self.remote_authority_mgr.with_write(|mgr| mgr.set_connected(authority));
    self.publish_remote_authority_event(authority.to_string(), AuthorityState::Connected);
    drained
  }

  /// Transitions the authority into quarantine using the provided duration or the configured
  /// default.
  pub fn remote_authority_set_quarantine(&self, authority: impl Into<String>, duration: Option<Duration>) {
    let authority = authority.into();
    let now_secs = self.monotonic_now().as_secs();
    let effective = duration.unwrap_or(self.default_quarantine_duration());
    self.remote_authority_mgr.with_write(|mgr| {
      mgr.set_quarantine(authority.clone(), now_secs, Some(effective));
    });
    let state = self.remote_authority_mgr.with_read(|mgr| mgr.state(&authority));
    self.publish_remote_authority_event(authority, state);
  }

  /// Handles an InvalidAssociation signal by moving the authority into quarantine.
  pub fn remote_authority_handle_invalid_association(&self, authority: impl Into<String>, duration: Option<Duration>) {
    let authority = authority.into();
    let now_secs = self.monotonic_now().as_secs();
    let effective = duration.unwrap_or(self.default_quarantine_duration());
    self.remote_authority_mgr.with_write(|mgr| {
      mgr.handle_invalid_association(authority.clone(), now_secs, Some(effective));
    });
    let state = self.remote_authority_mgr.with_read(|mgr| mgr.state(&authority));
    self.publish_remote_authority_event(authority, state);
  }

  /// Manually overrides a quarantined authority back to connected.
  pub fn remote_authority_manual_override_to_connected(&self, authority: &str) {
    self.remote_authority_mgr.with_write(|mgr| mgr.manual_override_to_connected(authority));
    self.publish_remote_authority_event(authority.to_string(), AuthorityState::Connected);
  }

  /// Defers a message while the authority is unresolved.
  ///
  /// # Errors
  ///
  /// Returns [`RemoteAuthorityError::Quarantined`] if the target authority is currently
  /// quarantined.
  pub fn remote_authority_defer(
    &self,
    authority: impl Into<String>,
    message: AnyMessageGeneric<TB>,
  ) -> Result<(), RemoteAuthorityError> {
    self.remote_authority_mgr.with_write(|mgr| mgr.defer_send(authority, message))
  }

  /// Attempts to defer a message, returning an error if the authority is quarantined.
  ///
  /// # Errors
  ///
  /// Returns [`RemoteAuthorityError::Quarantined`] when the authority remains quarantined.
  pub fn remote_authority_try_defer(
    &self,
    authority: impl Into<String>,
    message: AnyMessageGeneric<TB>,
  ) -> Result<(), RemoteAuthorityError> {
    self.remote_authority_mgr.with_write(|mgr| mgr.try_defer_send(authority, message))
  }

  /// Polls all authorities for expired quarantine windows and emits events for lifted entries.
  pub fn poll_remote_authorities(&self) {
    let now_secs = self.monotonic_now().as_secs();
    self.actor_path_registry.with_write(|registry| registry.poll_expired(now_secs));
    let lifted = self.remote_authority_mgr.with_write(|mgr| mgr.poll_quarantine_expiration(now_secs));
    for authority in lifted {
      self.publish_remote_authority_event(authority.clone(), AuthorityState::Unresolved);
    }
  }
}

impl<TB: RuntimeToolbox + 'static> Drop for SystemStateGeneric<TB> {
  fn drop(&mut self) {
    self.tick_driver_runtime.shutdown();
  }
}

impl<TB: RuntimeToolbox + 'static + Default> Default for SystemStateGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for SystemStateGeneric<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for SystemStateGeneric<TB> {}
