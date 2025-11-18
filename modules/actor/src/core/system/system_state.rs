//! Shared, mutable state owned by the actor system.

#[cfg(test)]
mod tests;

use alloc::{
  borrow::ToOwned,
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
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};
use hashbrown::HashMap;
use portable_atomic::{AtomicBool, AtomicU64, Ordering};

use super::{
  ActorPathRegistry, AuthorityState, GuardianKind, RemoteAuthorityError, RemoteAuthorityManagerGeneric, RemoteWatchHook,
};
use crate::core::{
  actor_prim::{
    ActorCellGeneric, Pid,
    actor_path::{ActorPath, ActorPathParser, ActorPathParts, ActorPathScheme, GuardianKind as PathGuardianKind},
    actor_ref::ActorRefGeneric,
  },
  config::{ActorSystemConfig, DispatchersGeneric, MailboxesGeneric},
  dead_letter::{DeadLetterEntryGeneric, DeadLetterGeneric, DeadLetterReason},
  error::{ActorError, SendError},
  event_stream::{EventStreamEvent, EventStreamGeneric, RemoteAuthorityEvent, TickDriverSnapshot},
  futures::ActorFuture,
  logging::{LogEvent, LogLevel},
  messaging::{AnyMessageGeneric, FailurePayload, SystemMessage},
  scheduler::{SchedulerContext, TaskRunSummary, TickDriverBootstrap, TickDriverRuntime},
  spawn::{NameRegistry, NameRegistryError, SpawnError},
  supervision::SupervisorDirective,
  system::{RegisterExtraTopLevelError, ReservationPolicy},
};

mod failure_outcome;

pub use failure_outcome::FailureOutcome;

/// Type alias for ask future collections.
type AskFutureVec<TB> = Vec<ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>>;

const RESERVED_TOP_LEVEL: [&str; 4] = ["user", "system", "temp", "deadLetters"];
const DEFAULT_SYSTEM_NAME: &str = "cellactor";
const DEFAULT_QUARANTINE_DURATION: Duration = Duration::from_secs(5 * 24 * 3600);

#[derive(Clone)]
struct PathIdentity {
  system_name:         String,
  canonical_host:      Option<String>,
  canonical_port:      Option<u16>,
  quarantine_duration: Duration,
  guardian_kind:       PathGuardianKind,
}

impl Default for PathIdentity {
  fn default() -> Self {
    Self {
      system_name:         DEFAULT_SYSTEM_NAME.to_string(),
      canonical_host:      None,
      canonical_port:      None,
      quarantine_duration: DEFAULT_QUARANTINE_DURATION,
      guardian_kind:       PathGuardianKind::User,
    }
  }
}

/// Captures global actor system state.
pub struct SystemStateGeneric<TB: RuntimeToolbox + 'static> {
  next_pid:               AtomicU64,
  clock:                  AtomicU64,
  cells:                  ToolboxMutex<HashMap<Pid, ArcShared<ActorCellGeneric<TB>>>, TB>,
  registries:             ToolboxMutex<HashMap<Option<Pid>, NameRegistry>, TB>,
  root_guardian:          ToolboxMutex<Option<ArcShared<ActorCellGeneric<TB>>>, TB>,
  system_guardian:        ToolboxMutex<Option<ArcShared<ActorCellGeneric<TB>>>, TB>,
  user_guardian:          ToolboxMutex<Option<ArcShared<ActorCellGeneric<TB>>>, TB>,
  ask_futures:            ToolboxMutex<AskFutureVec<TB>, TB>,
  termination:            ArcShared<ActorFuture<(), TB>>,
  terminated:             AtomicBool,
  terminating:            AtomicBool,
  root_started:           AtomicBool,
  event_stream:           ArcShared<EventStreamGeneric<TB>>,
  dead_letter:            ArcShared<DeadLetterGeneric<TB>>,
  extra_top_levels:       ToolboxMutex<HashMap<String, ActorRefGeneric<TB>>, TB>,
  temp_actors:            ToolboxMutex<HashMap<String, ActorRefGeneric<TB>>, TB>,
  temp_counter:           AtomicU64,
  failure_total:          AtomicU64,
  failure_restart_total:  AtomicU64,
  failure_stop_total:     AtomicU64,
  failure_escalate_total: AtomicU64,
  failure_inflight:       AtomicU64,
  extensions:             ToolboxMutex<HashMap<TypeId, ArcShared<dyn Any + Send + Sync + 'static>>, TB>,
  actor_ref_providers:    ToolboxMutex<HashMap<TypeId, ArcShared<dyn Any + Send + Sync + 'static>>, TB>,
  remote_watch_hook:      ToolboxMutex<Option<ArcShared<dyn RemoteWatchHook<TB>>>, TB>,
  dispatchers:            ArcShared<DispatchersGeneric<TB>>,
  mailboxes:              ArcShared<MailboxesGeneric<TB>>,
  path_identity:          ToolboxMutex<PathIdentity, TB>,
  actor_path_registry:    ToolboxMutex<ActorPathRegistry, TB>,
  remote_authority_mgr:   ArcShared<RemoteAuthorityManagerGeneric<TB>>,
  scheduler_context:      ToolboxMutex<Option<ArcShared<SchedulerContext<TB>>>, TB>,
  tick_driver_runtime:    ToolboxMutex<Option<TickDriverRuntime<TB>>, TB>,
}

/// Type alias for [SystemStateGeneric] with the default [NoStdToolbox].
pub type SystemState = SystemStateGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> SystemStateGeneric<TB> {
  /// Creates a fresh state container without any registered actors.
  #[must_use]
  pub fn new() -> Self {
    const DEAD_LETTER_CAPACITY: usize = 512;
    let event_stream = ArcShared::new(EventStreamGeneric::default());
    let dead_letter = ArcShared::new(DeadLetterGeneric::new(event_stream.clone(), DEAD_LETTER_CAPACITY));
    let dispatchers = ArcShared::new(DispatchersGeneric::new());
    dispatchers.ensure_default();
    let mailboxes = ArcShared::new(MailboxesGeneric::new());
    mailboxes.ensure_default();
    Self {
      next_pid: AtomicU64::new(0),
      clock: AtomicU64::new(0),
      cells: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()),
      registries: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()),
      root_guardian: <TB::MutexFamily as SyncMutexFamily>::create(None),
      system_guardian: <TB::MutexFamily as SyncMutexFamily>::create(None),
      user_guardian: <TB::MutexFamily as SyncMutexFamily>::create(None),
      ask_futures: <TB::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      termination: ArcShared::new(ActorFuture::<(), TB>::new()),
      terminated: AtomicBool::new(false),
      terminating: AtomicBool::new(false),
      root_started: AtomicBool::new(false),
      event_stream,
      dead_letter,
      extra_top_levels: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()),
      temp_actors: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()),
      temp_counter: AtomicU64::new(0),
      failure_total: AtomicU64::new(0),
      failure_restart_total: AtomicU64::new(0),
      failure_stop_total: AtomicU64::new(0),
      failure_escalate_total: AtomicU64::new(0),
      failure_inflight: AtomicU64::new(0),
      extensions: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()),
      actor_ref_providers: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()),
      remote_watch_hook: <TB::MutexFamily as SyncMutexFamily>::create(None),
      dispatchers,
      mailboxes,
      path_identity: <TB::MutexFamily as SyncMutexFamily>::create(PathIdentity::default()),
      actor_path_registry: <TB::MutexFamily as SyncMutexFamily>::create(ActorPathRegistry::new()),
      remote_authority_mgr: ArcShared::new(RemoteAuthorityManagerGeneric::new()),
      scheduler_context: <TB::MutexFamily as SyncMutexFamily>::create(None),
      tick_driver_runtime: <TB::MutexFamily as SyncMutexFamily>::create(None),
    }
  }

  /// Allocates a new unique [`Pid`] for an actor.
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    let value = self.next_pid.fetch_add(1, Ordering::Relaxed) + 1;
    Pid::new(value, 0)
  }

  /// Applies the actor system configuration (system name, remoting settings).
  pub fn apply_actor_system_config(&self, config: &ActorSystemConfig<TB>) {
    {
      let mut identity = self.path_identity.lock();
      identity.system_name = config.system_name().to_string();
      identity.guardian_kind = config.default_guardian();
      if let Some(remoting) = config.remoting_config() {
        identity.canonical_host = Some(remoting.canonical_host().to_string());
        identity.canonical_port = remoting.canonical_port();
        identity.quarantine_duration = remoting.quarantine_duration();
      } else {
        identity.canonical_host = None;
        identity.canonical_port = None;
        identity.quarantine_duration = DEFAULT_QUARANTINE_DURATION;
      }
    }

    let policy = ReservationPolicy::with_quarantine_duration(self.default_quarantine_duration());
    self.actor_path_registry.lock().set_policy(policy);
  }

  /// Registers the provided actor cell in the global registry.
  pub(crate) fn register_cell(&self, cell: ArcShared<ActorCellGeneric<TB>>) {
    let pid = cell.pid();
    self.cells.lock().insert(pid, cell);
    self.register_actor_path(pid);
  }

  /// Removes the actor cell associated with the pid.
  pub(crate) fn remove_cell(&self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    let reservation_source = {
      let registry = self.actor_path_registry.lock();
      registry.get(pid).map(|handle| (handle.canonical_uri().to_string(), handle.uid()))
    };

    if let Some((canonical, Some(uid))) = reservation_source
      && let Ok(actor_path) = ActorPathParser::parse(&canonical)
    {
      let now_secs = self.monotonic_now().as_secs();
      let mut registry = self.actor_path_registry.lock();
      let _ = registry.reserve_uid(&actor_path, uid, now_secs, None);
    }

    self.actor_path_registry.lock().unregister(pid);
    self.cells.lock().remove(pid)
  }

  fn register_actor_path(&self, pid: Pid) {
    if let Some(path) = self.canonical_actor_path(&pid) {
      self.actor_path_registry.lock().register(pid, &path);
    }
  }

  fn canonical_actor_path(&self, pid: &Pid) -> Option<ActorPath> {
    let base = self.actor_path(pid)?;
    let segments = base.segments().to_vec();
    let parts = self.canonical_parts();
    Some(ActorPath::from_parts_and_segments(parts, segments, base.uid()))
  }

  fn canonical_parts(&self) -> ActorPathParts {
    let identity = self.identity_snapshot();
    let mut parts = ActorPathParts::local(identity.system_name).with_guardian(identity.guardian_kind);
    if let Some(host) = identity.canonical_host {
      parts = parts.with_scheme(ActorPathScheme::FraktorTcp).with_authority_host(host);
      if let Some(port) = identity.canonical_port {
        parts = parts.with_authority_port(port);
      }
    }
    parts
  }

  fn identity_snapshot(&self) -> PathIdentity {
    self.path_identity.lock().clone()
  }

  fn default_quarantine_duration(&self) -> Duration {
    self.path_identity.lock().quarantine_duration
  }

  fn publish_remote_authority_event(&self, authority: String, state: AuthorityState) {
    let event = RemoteAuthorityEvent::new(authority, state);
    self.event_stream.publish(&EventStreamEvent::RemoteAuthority(event));
  }

  /// Retrieves an actor cell by pid.
  #[must_use]
  pub(crate) fn cell(&self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.cells.lock().get(pid).cloned()
  }

  /// Binds an actor name within its parent's scope.
  ///
  /// # Errors
  ///
  /// Returns an error if the requested name is already taken.
  pub(crate) fn assign_name(&self, parent: Option<Pid>, hint: Option<&str>, pid: Pid) -> Result<String, SpawnError> {
    let mut registries = self.registries.lock();
    let registry = registries.entry(parent).or_insert_with(NameRegistry::new);

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
  pub(crate) fn release_name(&self, parent: Option<Pid>, name: &str) {
    if let Some(registry) = self.registries.lock().get_mut(&parent) {
      registry.remove(name);
    }
  }

  /// Stores the root guardian cell reference.
  pub(crate) fn set_root_guardian(&self, cell: ArcShared<ActorCellGeneric<TB>>) {
    *self.root_guardian.lock() = Some(cell);
  }

  /// Stores the system guardian cell reference.
  pub(crate) fn set_system_guardian(&self, cell: ArcShared<ActorCellGeneric<TB>>) {
    *self.system_guardian.lock() = Some(cell);
  }

  /// Stores the user guardian cell reference.
  pub(crate) fn set_user_guardian(&self, cell: ArcShared<ActorCellGeneric<TB>>) {
    *self.user_guardian.lock() = Some(cell);
  }

  /// Clears the guardian slot matching the pid and returns which guardian stopped.
  pub(crate) fn clear_guardian(&self, pid: Pid) -> Option<GuardianKind> {
    if Self::clear_specific_guardian(&self.root_guardian, pid) {
      return Some(GuardianKind::Root);
    }
    if Self::clear_specific_guardian(&self.system_guardian, pid) {
      return Some(GuardianKind::System);
    }
    if Self::clear_specific_guardian(&self.user_guardian, pid) {
      return Some(GuardianKind::User);
    }
    None
  }

  fn clear_specific_guardian(slot: &ToolboxMutex<Option<ArcShared<ActorCellGeneric<TB>>>, TB>, pid: Pid) -> bool {
    let mut guard = slot.lock();
    if guard.as_ref().map(|cell| cell.pid()) == Some(pid) {
      *guard = None;
      return true;
    }
    false
  }

  /// Returns the root guardian cell if initialised.
  #[must_use]
  #[allow(dead_code)]
  pub(crate) fn root_guardian(&self) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.root_guardian.lock().clone()
  }

  /// Returns the system guardian cell if initialised.
  #[must_use]
  pub(crate) fn system_guardian(&self) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.system_guardian.lock().clone()
  }

  /// Returns the user guardian cell if initialised.
  #[must_use]
  pub(crate) fn user_guardian(&self) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.user_guardian.lock().clone()
  }

  /// Returns the pid of the root guardian if available.
  #[must_use]
  pub fn root_guardian_pid(&self) -> Option<Pid> {
    self.root_guardian.lock().as_ref().map(|cell| cell.pid())
  }

  /// Returns the pid of the system guardian if available.
  #[must_use]
  pub fn system_guardian_pid(&self) -> Option<Pid> {
    self.system_guardian.lock().as_ref().map(|cell| cell.pid())
  }

  /// Returns the pid of the user guardian if available.
  #[must_use]
  pub fn user_guardian_pid(&self) -> Option<Pid> {
    self.user_guardian.lock().as_ref().map(|cell| cell.pid())
  }

  /// Registers an extra top-level path prior to root startup.
  pub(crate) fn register_extra_top_level(
    &self,
    name: &str,
    actor: ActorRefGeneric<TB>,
  ) -> Result<(), RegisterExtraTopLevelError> {
    if self.root_started.load(Ordering::Acquire) {
      return Err(RegisterExtraTopLevelError::AlreadyStarted);
    }
    if name.is_empty() || RESERVED_TOP_LEVEL.iter().any(|reserved| reserved.eq_ignore_ascii_case(name)) {
      return Err(RegisterExtraTopLevelError::ReservedName(name.into()));
    }
    let mut registry = self.extra_top_levels.lock();
    if registry.contains_key(name) {
      return Err(RegisterExtraTopLevelError::DuplicateName(name.into()));
    }
    registry.insert(name.into(), actor);
    Ok(())
  }

  /// Returns a registered extra top-level reference if present.
  #[must_use]
  pub fn extra_top_level(&self, name: &str) -> Option<ActorRefGeneric<TB>> {
    self.extra_top_levels.lock().get(name).cloned()
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

  /// Generates a unique `/temp` path segment and registers the supplied actor reference.
  #[must_use]
  pub(crate) fn register_temp_actor(&self, actor: ActorRefGeneric<TB>) -> String {
    let id = self.temp_counter.fetch_add(1, Ordering::Relaxed) + 1;
    let name = format!("t{:x}", id);
    self.temp_actors.lock().insert(name.clone(), actor);
    name
  }

  /// Removes a temporary actor reference if registered.
  pub(crate) fn unregister_temp_actor(&self, name: &str) -> Option<ActorRefGeneric<TB>> {
    self.temp_actors.lock().remove(name)
  }

  /// Resolves a registered temporary actor reference.
  #[must_use]
  pub(crate) fn temp_actor(&self, name: &str) -> Option<ActorRefGeneric<TB>> {
    self.temp_actors.lock().get(name).cloned()
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
    segments.pop(); // discard root
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
  pub fn event_stream(&self) -> ArcShared<EventStreamGeneric<TB>> {
    self.event_stream.clone()
  }

  /// Returns a snapshot of deadletter entries.
  #[must_use]
  pub fn dead_letters(&self) -> Vec<DeadLetterEntryGeneric<TB>> {
    self.dead_letter.entries()
  }

  /// Registers an ask future so the actor system can track its completion.
  pub(crate) fn register_ask_future(&self, future: ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>) {
    self.ask_futures.lock().push(future);
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

  /// Returns `true` when an extension for the provided [`TypeId`] is registered.
  pub(crate) fn has_extension(&self, type_id: TypeId) -> bool {
    self.extensions.lock().contains_key(&type_id)
  }

  /// Returns an extension by [`TypeId`].
  pub(crate) fn extension<E>(&self, type_id: TypeId) -> Option<ArcShared<E>>
  where
    E: Any + Send + Sync + 'static, {
    self.extensions.lock().get(&type_id).cloned().and_then(|handle| handle.downcast::<E>().ok())
  }

  /// Inserts an extension if absent and returns the shared instance.
  pub(crate) fn extension_or_insert_with<E, F>(&self, type_id: TypeId, factory: F) -> ArcShared<E>
  where
    E: Any + Send + Sync + 'static,
    F: FnOnce() -> ArcShared<E>, {
    let mut guard = self.extensions.lock();
    if let Some(existing) = guard.get(&type_id) {
      if let Ok(extension) = existing.clone().downcast::<E>() {
        return extension;
      }
      panic!("extension type mismatch for id {type_id:?}");
    }
    let extension = factory();
    let erased: ArcShared<dyn Any + Send + Sync + 'static> = extension.clone();
    guard.insert(type_id, erased);
    extension
  }

  pub(crate) fn extension_by_type<E>(&self) -> Option<ArcShared<E>>
  where
    E: Any + Send + Sync + 'static, {
    let guard = self.extensions.lock();
    for handle in guard.values() {
      if let Ok(extension) = handle.clone().downcast::<E>() {
        return Some(extension);
      }
    }
    None
  }

  pub(crate) fn install_actor_ref_provider<P>(&self, provider: ArcShared<P>)
  where
    P: Any + Send + Sync + 'static, {
    let erased: ArcShared<dyn Any + Send + Sync + 'static> = provider;
    self.actor_ref_providers.lock().insert(TypeId::of::<P>(), erased);
  }

  pub(crate) fn register_remote_watch_hook(&self, hook: ArcShared<dyn RemoteWatchHook<TB>>) {
    let mut guard = self.remote_watch_hook.lock();
    *guard = Some(hook);
  }

  pub(crate) fn actor_ref_provider<P>(&self) -> Option<ArcShared<P>>
  where
    P: Any + Send + Sync + 'static, {
    self.actor_ref_providers.lock().get(&TypeId::of::<P>()).cloned().and_then(|provider| provider.downcast::<P>().ok())
  }

  fn remote_watch_hook(&self) -> Option<ArcShared<dyn RemoteWatchHook<TB>>> {
    self.remote_watch_hook.lock().clone()
  }

  fn forward_remote_watch(&self, target: Pid, watcher: Pid) -> bool {
    self.remote_watch_hook().is_some_and(|hook| hook.handle_watch(target, watcher))
  }

  fn forward_remote_unwatch(&self, target: Pid, watcher: Pid) -> bool {
    self.remote_watch_hook().is_some_and(|hook| hook.handle_unwatch(target, watcher))
  }

  /// Registers a child under the specified parent pid.
  pub(crate) fn register_child(&self, parent: Pid, child: Pid) {
    if let Some(cell) = self.cell(&parent) {
      cell.register_child(child);
    }
  }

  /// Removes a child from its parent's supervision registry.
  pub(crate) fn unregister_child(&self, parent: Option<Pid>, child: Pid) {
    if let Some(parent_pid) = parent
      && let Some(cell) = self.cell(&parent_pid)
    {
      cell.unregister_child(&child);
    }
  }

  /// Returns the children supervised by the specified parent pid.
  #[must_use]
  pub(crate) fn child_pids(&self, parent: Pid) -> Vec<Pid> {
    self.cell(&parent).map_or_else(Vec::new, |cell| cell.children())
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

  /// Records an explicit deadletter entry originating from runtime logic.
  pub(crate) fn record_dead_letter(
    &self,
    message: AnyMessageGeneric<TB>,
    reason: DeadLetterReason,
    target: Option<Pid>,
  ) {
    let timestamp = self.monotonic_now();
    self.dead_letter.record_entry(message, reason, target, timestamp);
  }

  /// Marks the system as terminated and completes the termination future.
  pub(crate) fn mark_terminated(&self) {
    self.terminating.store(true, Ordering::Release);
    if self.terminated.swap(true, Ordering::AcqRel) {
      return;
    }
    self.termination.complete(());
  }

  /// Returns a future that resolves once the actor system terminates.
  #[must_use]
  pub(crate) fn termination_future(&self) -> ArcShared<ActorFuture<(), TB>> {
    self.termination.clone()
  }

  /// Drains ask futures that have completed since the previous inspection.
  pub(crate) fn drain_ready_ask_futures(&self) -> Vec<ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>> {
    let mut registry = self.ask_futures.lock();
    let mut ready = Vec::new();
    let mut index = 0_usize;

    while index < registry.len() {
      if registry[index].is_ready() {
        ready.push(registry.swap_remove(index));
      } else {
        index += 1;
      }
    }

    ready
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

  /// Installs the scheduler service handle.
  pub fn install_scheduler_context(&self, context: ArcShared<SchedulerContext<TB>>) {
    let mut guard = self.scheduler_context.lock();
    guard.replace(context);
  }

  /// Returns the scheduler context when it has been initialized.
  #[must_use]
  pub fn scheduler_context(&self) -> Option<ArcShared<SchedulerContext<TB>>> {
    self.scheduler_context.lock().clone()
  }

  /// Installs the tick driver runtime.
  pub fn install_tick_driver_runtime(&self, runtime: TickDriverRuntime<TB>) {
    let mut guard = self.tick_driver_runtime.lock();
    guard.replace(runtime);
  }

  /// Returns the tick driver runtime when it has been initialized.
  #[must_use]
  pub fn tick_driver_runtime(&self) -> Option<TickDriverRuntime<TB>> {
    self.tick_driver_runtime.lock().as_ref().cloned()
  }

  /// Returns the last recorded tick driver snapshot when available.
  #[must_use]
  pub fn tick_driver_snapshot(&self) -> Option<TickDriverSnapshot> {
    self.scheduler_context().and_then(|context| context.driver_snapshot())
  }

  /// Shuts down the scheduler context if configured.
  pub fn shutdown_scheduler(&self) -> Option<TaskRunSummary> {
    self.scheduler_context().map(|context| context.shutdown())
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
  fn handle_failure(&self, pid: Pid, parent: Option<Pid>, error: &ActorError) {
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
  pub const fn actor_path_registry(&self) -> &ToolboxMutex<ActorPathRegistry, TB> {
    &self.actor_path_registry
  }

  /// Returns a reference to the RemoteAuthorityManager.
  #[must_use]
  pub const fn remote_authority_manager(&self) -> &ArcShared<RemoteAuthorityManagerGeneric<TB>> {
    &self.remote_authority_mgr
  }

  /// Returns the current authority state.
  #[must_use]
  pub fn remote_authority_state(&self, authority: &str) -> AuthorityState {
    self.remote_authority_mgr.state(authority)
  }

  /// Returns a snapshot of known remote authorities and their states.
  pub fn remote_authority_snapshots(&self) -> Vec<(String, AuthorityState)> {
    self.remote_authority_mgr.snapshots()
  }

  /// Marks the authority as connected and emits an event.
  pub fn remote_authority_set_connected(&self, authority: &str) -> Option<VecDeque<AnyMessageGeneric<TB>>> {
    let drained = self.remote_authority_mgr.set_connected(authority);
    self.publish_remote_authority_event(authority.to_string(), AuthorityState::Connected);
    drained
  }

  /// Transitions the authority into quarantine using the provided duration or the configured
  /// default.
  pub fn remote_authority_set_quarantine(&self, authority: impl Into<String>, duration: Option<Duration>) {
    let authority = authority.into();
    let now_secs = self.monotonic_now().as_secs();
    let effective = duration.unwrap_or(self.default_quarantine_duration());
    self.remote_authority_mgr.set_quarantine(authority.clone(), now_secs, Some(effective));
    let state = self.remote_authority_mgr.state(&authority);
    self.publish_remote_authority_event(authority, state);
  }

  /// Handles an InvalidAssociation signal by moving the authority into quarantine.
  pub fn remote_authority_handle_invalid_association(&self, authority: impl Into<String>, duration: Option<Duration>) {
    let authority = authority.into();
    let now_secs = self.monotonic_now().as_secs();
    let effective = duration.unwrap_or(self.default_quarantine_duration());
    self.remote_authority_mgr.handle_invalid_association(authority.clone(), now_secs, Some(effective));
    let state = self.remote_authority_mgr.state(&authority);
    self.publish_remote_authority_event(authority, state);
  }

  /// Manually overrides a quarantined authority back to connected.
  pub fn remote_authority_manual_override_to_connected(&self, authority: &str) {
    self.remote_authority_mgr.manual_override_to_connected(authority);
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
    self.remote_authority_mgr.defer_send(authority, message)
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
    self.remote_authority_mgr.try_defer_send(authority, message)
  }

  /// Polls all authorities for expired quarantine windows and emits events for lifted entries.
  pub fn poll_remote_authorities(&self) {
    let now_secs = self.monotonic_now().as_secs();
    self.actor_path_registry.lock().poll_expired(now_secs);
    let lifted = self.remote_authority_mgr.poll_quarantine_expiration(now_secs);
    for authority in lifted {
      self.publish_remote_authority_event(authority.clone(), AuthorityState::Unresolved);
    }
  }
}

impl<TB: RuntimeToolbox + 'static> Drop for SystemStateGeneric<TB> {
  fn drop(&mut self) {
    if let Some(runtime) = self.tick_driver_runtime.lock().take() {
      TickDriverBootstrap::shutdown(runtime.driver());
    }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for SystemStateGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for SystemStateGeneric<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for SystemStateGeneric<TB> {}
