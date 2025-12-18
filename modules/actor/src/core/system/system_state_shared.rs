//! Shared wrapper for system state.

#[cfg(test)]
mod tests;

use alloc::{
  collections::VecDeque,
  format,
  string::{String, ToString},
  vec::Vec,
};
use core::{any::TypeId, time::Duration};

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncRwLockFamily, ToolboxRwLock},
  sync::{ArcShared, SharedAccess, sync_rwlock_like::SyncRwLockLike},
};

use super::{
  ActorPathRegistry, ActorRefProvider, ActorRefProviderSharedGeneric, AuthorityState, CellsSharedGeneric, GuardianKind,
  RemoteAuthorityError, RemoteWatchHookDynSharedGeneric, RemotingConfig, SystemStateGeneric,
};
use crate::core::{
  actor_prim::{
    ActorCellGeneric, Pid,
    actor_path::{ActorPath, ActorPathParser, ActorPathParts, ActorPathScheme, GuardianKind as PathGuardianKind},
    actor_ref::ActorRefGeneric,
  },
  dead_letter::{DeadLetterEntryGeneric, DeadLetterReason, DeadLetterSharedGeneric},
  dispatcher::{DispatcherConfigGeneric, DispatcherRegistryError},
  error::{ActorError, SendError},
  event_stream::{EventStreamEvent, EventStreamSharedGeneric, TickDriverSnapshot},
  futures::ActorFutureSharedGeneric,
  logging::{LogEvent, LogLevel},
  mailbox::MailboxRegistryError,
  messaging::{AnyMessageGeneric, FailurePayload, SystemMessage},
  props::MailboxConfig,
  scheduler::{SchedulerContextSharedGeneric, TaskRunSummary, TickDriverRuntime},
  spawn::SpawnError,
  supervision::SupervisorDirective,
  system::{ActorSystemBuildError, RegisterExtensionError, RegisterExtraTopLevelError},
};

/// Shared wrapper for [`SystemStateGeneric`] providing thread-safe access.
///
/// This wrapper uses a read-write lock to provide safe concurrent access
/// to the underlying system state.
pub struct SystemStateSharedGeneric<TB: RuntimeToolbox + 'static> {
  pub(crate) inner:    ArcShared<ToolboxRwLock<SystemStateGeneric<TB>, TB>>,
  system_name:         String,
  guardian_kind:       PathGuardianKind,
  canonical_host:      Option<String>,
  canonical_port:      Option<u16>,
  quarantine_duration: Duration,
  event_stream:        EventStreamSharedGeneric<TB>,
  dead_letter:         DeadLetterSharedGeneric<TB>,
  cells:               CellsSharedGeneric<TB>,
  termination:         ActorFutureSharedGeneric<(), TB>,
  remote_watch_hook:   RemoteWatchHookDynSharedGeneric<TB>,
  scheduler_ctx:       SchedulerContextSharedGeneric<TB>,
  tick_driver_rt:      TickDriverRuntime<TB>,
}

impl<TB: RuntimeToolbox + 'static> Clone for SystemStateSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self {
      inner:               self.inner.clone(),
      system_name:         self.system_name.clone(),
      guardian_kind:       self.guardian_kind,
      canonical_host:      self.canonical_host.clone(),
      canonical_port:      self.canonical_port,
      quarantine_duration: self.quarantine_duration,
      event_stream:        self.event_stream.clone(),
      dead_letter:         self.dead_letter.clone(),
      cells:               self.cells.clone(),
      termination:         self.termination.clone(),
      remote_watch_hook:   self.remote_watch_hook.clone(),
      scheduler_ctx:       self.scheduler_ctx.clone(),
      tick_driver_rt:      self.tick_driver_rt.clone(),
    }
  }
}

impl<TB: RuntimeToolbox + 'static> SystemStateSharedGeneric<TB> {
  /// Creates a new shared system state.
  #[must_use]
  pub fn new(state: SystemStateGeneric<TB>) -> Self {
    let system_name = state.system_name();
    let guardian_kind = state.path_guardian_kind();
    let canonical_host = state.canonical_host();
    let canonical_port = state.canonical_port();
    let quarantine_duration = state.quarantine_duration();
    let event_stream = state.event_stream();
    let dead_letter = state.dead_letter_store();
    let cells = state.cells_handle();
    let termination = state.termination_future();
    let remote_watch_hook = state.remote_watch_hook_handle();
    let scheduler_ctx = state.scheduler_context();
    let tick_driver_rt = state.tick_driver_runtime();
    let inner = ArcShared::new(<TB::RwLockFamily as SyncRwLockFamily>::create(state));
    Self {
      inner,
      system_name,
      guardian_kind,
      canonical_host,
      canonical_port,
      quarantine_duration,
      event_stream,
      dead_letter,
      cells,
      termination,
      remote_watch_hook,
      scheduler_ctx,
      tick_driver_rt,
    }
  }

  /// Creates a shared wrapper from an existing [`ArcShared`].
  #[must_use]
  pub(crate) fn from_arc_shared(inner: ArcShared<ToolboxRwLock<SystemStateGeneric<TB>, TB>>) -> Self {
    let guard = inner.read();
    let system_name = guard.system_name();
    let guardian_kind = guard.path_guardian_kind();
    let canonical_host = guard.canonical_host();
    let canonical_port = guard.canonical_port();
    let quarantine_duration = guard.quarantine_duration();
    let event_stream = guard.event_stream();
    let dead_letter = guard.dead_letter_store();
    let cells = guard.cells_handle();
    let termination = guard.termination_future();
    let remote_watch_hook = guard.remote_watch_hook_handle();
    let scheduler_ctx = guard.scheduler_context();
    let tick_driver_rt = guard.tick_driver_runtime();
    drop(guard);
    Self {
      inner,
      system_name,
      guardian_kind,
      canonical_host,
      canonical_port,
      quarantine_duration,
      event_stream,
      dead_letter,
      cells,
      termination,
      remote_watch_hook,
      scheduler_ctx,
      tick_driver_rt,
    }
  }

  /// Returns the inner reference for direct access when needed.
  #[must_use]
  pub const fn inner(&self) -> &ArcShared<ToolboxRwLock<SystemStateGeneric<TB>, TB>> {
    &self.inner
  }

  /// Creates a weak reference to this system state.
  #[must_use]
  pub fn downgrade(&self) -> super::SystemStateWeakGeneric<TB> {
    super::SystemStateWeakGeneric { inner: self.inner.downgrade() }
  }

  // ====== SystemStateGeneric の委譲メソッド ======

  /// Allocates a new unique [`Pid`] for an actor.
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    self.inner.read().allocate_pid()
  }

  /// Registers the provided actor cell in the global registry.
  pub fn register_cell(&self, cell: ArcShared<ActorCellGeneric<TB>>) {
    let pid = cell.pid();
    self.cells.with_write(|cells| cells.insert(pid, cell));
    if let Some(path) = self.canonical_actor_path(&pid) {
      self.inner.write().actor_path_registry_mut().register(pid, &path);
    }
  }

  /// Removes the actor cell associated with the pid.
  pub fn remove_cell(&self, pid: &Pid) {
    let reservation_source =
      self.inner.read().actor_path_registry().get(pid).map(|handle| (handle.canonical_uri().to_string(), handle.uid()));

    if let Some((canonical, Some(uid))) = reservation_source
      && let Ok(actor_path) = ActorPathParser::parse(&canonical)
    {
      let now_secs = self.monotonic_now().as_secs();
      let mut guard = self.inner.write();
      let _ = guard.actor_path_registry_mut().reserve_uid(&actor_path, uid, now_secs, None);
      guard.actor_path_registry_mut().unregister(pid);
      drop(guard);
      let _ = self.cells.with_write(|cells| cells.remove(pid));
      return;
    }

    self.inner.write().actor_path_registry_mut().unregister(pid);
    let _ = self.cells.with_write(|cells| cells.remove(pid));
  }

  /// Returns the canonical actor path for the given pid when available.
  #[must_use]
  pub fn canonical_actor_path(&self, pid: &Pid) -> Option<ActorPath> {
    let base = self.actor_path(pid)?;
    let segments = base.segments().to_vec();
    let parts = self.canonical_parts()?;
    Some(ActorPath::from_parts_and_segments(parts, segments, base.uid()))
  }

  fn canonical_parts(&self) -> Option<ActorPathParts> {
    let mut parts = ActorPathParts::local(self.system_name.clone()).with_guardian(self.guardian_kind);
    let Some(host) = self.canonical_host.clone() else {
      return Some(parts);
    };
    let port = self.canonical_port?;
    parts = parts.with_scheme(ActorPathScheme::FraktorTcp).with_authority_host(host).with_authority_port(port);
    Some(parts)
  }

  /// Returns the configured canonical host/port pair when remoting is enabled.
  #[must_use]
  pub fn canonical_authority_components(&self) -> Option<(String, Option<u16>)> {
    match (&self.canonical_host, self.canonical_port) {
      | (Some(host), Some(port)) => Some((host.clone(), Some(port))),
      | _ => None,
    }
  }

  /// Returns true when canonical_host is set but canonical_port is missing.
  #[must_use]
  pub const fn has_partial_canonical_authority(&self) -> bool {
    self.canonical_host.is_some() && self.canonical_port.is_none()
  }

  /// Returns the canonical authority string.
  #[must_use]
  pub fn canonical_authority_endpoint(&self) -> Option<String> {
    self.canonical_authority_components().map(|(host, port)| match port {
      | Some(port) => format!("{host}:{port}"),
      | None => host,
    })
  }

  /// Returns the configured actor system name.
  #[must_use]
  pub fn system_name(&self) -> String {
    self.system_name.clone()
  }

  /// Retrieves an actor cell by pid.
  #[must_use]
  pub fn cell(&self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.cells.with_read(|cells| cells.get(pid))
  }

  /// Binds an actor name within its parent's scope.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] if the name assignment fails.
  pub fn assign_name(&self, parent: Option<Pid>, hint: Option<&str>, pid: Pid) -> Result<String, SpawnError> {
    self.inner.write().assign_name(parent, hint, pid)
  }

  /// Releases the association between a name and its pid in the registry.
  pub fn release_name(&self, parent: Option<Pid>, name: &str) {
    self.inner.write().release_name(parent, name);
  }

  /// Stores the root guardian cell reference.
  pub(crate) fn set_root_guardian(&self, cell: &ArcShared<ActorCellGeneric<TB>>) {
    self.inner.write().set_root_guardian(cell);
  }

  /// Stores the system guardian cell reference.
  pub(crate) fn set_system_guardian(&self, cell: &ArcShared<ActorCellGeneric<TB>>) {
    self.inner.write().set_system_guardian(cell);
  }

  /// Stores the user guardian cell reference.
  pub(crate) fn set_user_guardian(&self, cell: &ArcShared<ActorCellGeneric<TB>>) {
    self.inner.write().set_user_guardian(cell);
  }

  /// Returns the guardian kind matching the provided pid when registered.
  #[must_use]
  pub fn guardian_kind_by_pid(&self, pid: Pid) -> Option<GuardianKind> {
    self.inner.read().guardian_kind_by_pid(pid)
  }

  /// Marks the specified guardian as stopped.
  pub fn mark_guardian_stopped(&self, kind: GuardianKind) {
    self.inner.read().mark_guardian_stopped(kind);
  }

  /// Returns the root guardian cell if initialised.
  #[must_use]
  pub fn root_guardian(&self) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.inner.read().root_guardian()
  }

  /// Returns the system guardian cell if initialised.
  #[must_use]
  pub fn system_guardian(&self) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.inner.read().system_guardian()
  }

  /// Returns the user guardian cell if initialised.
  #[must_use]
  pub fn user_guardian(&self) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.inner.read().user_guardian()
  }

  /// Returns the pid of the root guardian if available.
  #[must_use]
  pub fn root_guardian_pid(&self) -> Option<Pid> {
    self.inner.read().root_guardian_pid()
  }

  /// Returns the pid of the system guardian if available.
  #[must_use]
  pub fn system_guardian_pid(&self) -> Option<Pid> {
    self.inner.read().system_guardian_pid()
  }

  /// Returns the pid of the user guardian if available.
  #[must_use]
  pub fn user_guardian_pid(&self) -> Option<Pid> {
    self.inner.read().user_guardian_pid()
  }

  /// Returns the PID registered for the specified guardian.
  #[must_use]
  pub fn guardian_pid(&self, kind: GuardianKind) -> Option<Pid> {
    self.inner.read().guardian_pid(kind)
  }

  /// Registers a PID for the specified guardian kind.
  #[cfg(any(test, feature = "test-support"))]
  pub(crate) fn register_guardian_pid(&self, kind: GuardianKind, pid: Pid) {
    self.inner.write().register_guardian_pid(kind, pid);
  }

  /// Returns whether the specified guardian is alive.
  #[must_use]
  pub fn guardian_alive(&self, kind: GuardianKind) -> bool {
    self.inner.read().guardian_alive(kind)
  }

  /// Registers an extra top-level path prior to root startup.
  ///
  /// # Errors
  ///
  /// Returns [`RegisterExtraTopLevelError`] if the registration fails.
  pub fn register_extra_top_level(
    &self,
    name: &str,
    actor: ActorRefGeneric<TB>,
  ) -> Result<(), RegisterExtraTopLevelError> {
    if self.inner.read().has_root_started() {
      return Err(RegisterExtraTopLevelError::AlreadyStarted);
    }
    self.inner.write().register_extra_top_level(name, actor)
  }

  /// Returns a registered extra top-level reference if present.
  #[must_use]
  pub fn extra_top_level(&self, name: &str) -> Option<ActorRefGeneric<TB>> {
    self.inner.read().extra_top_level(name)
  }

  /// Marks the root guardian as fully initialised.
  pub fn mark_root_started(&self) {
    self.inner.read().mark_root_started();
  }

  /// Indicates whether the root guardian has completed startup.
  #[must_use]
  pub fn has_root_started(&self) -> bool {
    self.inner.read().has_root_started()
  }

  /// Attempts to transition the system into the terminating state.
  #[must_use]
  pub fn begin_termination(&self) -> bool {
    self.inner.read().begin_termination()
  }

  /// Indicates whether the system is currently terminating.
  #[must_use]
  pub fn is_terminating(&self) -> bool {
    self.inner.read().is_terminating()
  }

  /// Generates a unique `/temp` path segment and registers the supplied actor reference.
  #[must_use]
  pub fn register_temp_actor(&self, actor: ActorRefGeneric<TB>) -> String {
    self.inner.write().register_temp_actor(actor)
  }

  /// Removes a temporary actor reference if registered.
  pub fn unregister_temp_actor(&self, name: &str) {
    self.inner.write().unregister_temp_actor(name);
  }

  /// Resolves a registered temporary actor reference.
  #[must_use]
  pub fn temp_actor(&self, name: &str) -> Option<ActorRefGeneric<TB>> {
    self.inner.read().temp_actor(name)
  }

  /// Resolves the actor path for the specified pid if the actor exists.
  #[must_use]
  pub fn actor_path(&self, pid: &Pid) -> Option<ActorPath> {
    let cell = self.cell(pid)?;
    let mut segments = Vec::new();
    let mut current = Some(cell);
    while let Some(cursor) = current {
      segments.push(cursor.name().to_string());
      current = cursor.parent().and_then(|parent_pid| self.cell(&parent_pid));
    }
    if segments.is_empty() {
      return Some(ActorPath::root_with_guardian(self.guardian_kind));
    }
    segments.pop(); // ルート要素を捨てる
    if segments.is_empty() {
      return Some(ActorPath::root_with_guardian(self.guardian_kind));
    }
    segments.reverse();
    let mut path = ActorPath::root_with_guardian(self.guardian_kind);
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

  /// Registers an ask future so the actor system can track its completion.
  pub fn register_ask_future(&self, future: ActorFutureSharedGeneric<AnyMessageGeneric<TB>, TB>) {
    self.inner.write().register_ask_future(future);
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
  #[must_use]
  pub fn has_extension(&self, type_id: TypeId) -> bool {
    self.inner.read().has_extension(type_id)
  }

  /// Returns an extension by [`TypeId`].
  #[must_use]
  pub fn extension<E>(&self, type_id: TypeId) -> Option<ArcShared<E>>
  where
    E: core::any::Any + Send + Sync + 'static, {
    self.inner.read().extension(type_id)
  }

  /// Inserts an extension if absent and returns the shared instance.
  ///
  /// # Errors
  ///
  /// Returns [`RegisterExtensionError::AlreadyStarted`] when the actor system already finished
  /// startup and the extension is not registered yet.
  ///
  /// # Panics
  ///
  /// Panics when an extension exists for `type_id` but cannot be downcast to `E`.
  pub fn extension_or_insert_with<E, F>(
    &self,
    type_id: TypeId,
    factory: F,
  ) -> Result<ArcShared<E>, RegisterExtensionError>
  where
    E: core::any::Any + Send + Sync + 'static,
    F: FnOnce() -> ArcShared<E>, {
    {
      let guard = self.inner.read();
      if let Some(existing) = guard.extension_raw(&type_id) {
        if let Ok(extension) = existing.downcast::<E>() {
          return Ok(extension);
        }
        panic!("extension type mismatch for id {type_id:?}");
      }
      if guard.has_root_started() && guard.root_guardian_pid().is_some() {
        return Err(RegisterExtensionError::AlreadyStarted);
      }
    }

    let created = factory();
    let erased: ArcShared<dyn core::any::Any + Send + Sync + 'static> = created.clone();

    let mut guard = self.inner.write();
    if let Some(existing) = guard.extension_raw(&type_id) {
      if let Ok(extension) = existing.downcast::<E>() {
        return Ok(extension);
      }
      panic!("extension type mismatch for id {type_id:?}");
    }
    if guard.has_root_started() && guard.root_guardian_pid().is_some() {
      return Err(RegisterExtensionError::AlreadyStarted);
    }
    guard.insert_extension(type_id, erased);
    Ok(created)
  }

  /// Returns an extension by its type.
  #[must_use]
  pub fn extension_by_type<E>(&self) -> Option<ArcShared<E>>
  where
    E: core::any::Any + Send + Sync + 'static, {
    self.inner.read().extension_by_type()
  }

  /// Installs an actor ref provider.
  ///
  /// # Errors
  ///
  /// Returns [`ActorSystemBuildError::Configuration`] when called after system startup.
  pub fn install_actor_ref_provider<P>(
    &self,
    provider: &ActorRefProviderSharedGeneric<TB, P>,
  ) -> Result<(), ActorSystemBuildError>
  where
    P: ActorRefProvider<TB> + core::any::Any + Send + Sync + 'static, {
    let mut guard = self.inner.write();
    if guard.has_root_started() {
      return Err(ActorSystemBuildError::Configuration(
        "actor-ref provider registration is only allowed before system startup".into(),
      ));
    }
    guard.install_actor_ref_provider(provider);
    Ok(())
  }

  /// Registers a remote watch hook.
  pub fn register_remote_watch_hook(&self, hook: alloc::boxed::Box<dyn super::RemoteWatchHook<TB>>) {
    self.remote_watch_hook.replace(hook);
  }

  /// Returns an actor ref provider.
  #[must_use]
  pub fn actor_ref_provider<P>(&self) -> Option<ActorRefProviderSharedGeneric<TB, P>>
  where
    P: ActorRefProvider<TB> + core::any::Any + Send + Sync + 'static, {
    self.inner.read().actor_ref_provider()
  }

  /// Invokes a provider registered for the given scheme.
  #[must_use]
  pub fn actor_ref_provider_call_for_scheme(
    &self,
    scheme: ActorPathScheme,
    path: ActorPath,
  ) -> Option<Result<ActorRefGeneric<TB>, ActorError>> {
    let caller = {
      let guard = self.inner.read();
      guard.actor_ref_provider_caller_for_scheme(scheme)?
    };
    Some(caller(path))
  }

  /// Registers a child under the specified parent pid.
  pub fn register_child(&self, parent: Pid, child: Pid) {
    if let Some(cell) = self.cell(&parent) {
      cell.register_child(child);
    }
  }

  /// Removes a child from its parent's supervision registry.
  pub fn unregister_child(&self, parent: Option<Pid>, child: Pid) {
    if let Some(parent_pid) = parent
      && let Some(cell) = self.cell(&parent_pid)
    {
      cell.unregister_child(&child);
    }
  }

  /// Returns the children supervised by the specified parent pid.
  #[must_use]
  pub fn child_pids(&self, parent: Pid) -> Vec<Pid> {
    self.cell(&parent).map_or_else(Vec::new, |cell| cell.children())
  }

  /// Sends a system message to the specified actor.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] if the message cannot be delivered.
  pub fn send_system_message(&self, pid: Pid, message: SystemMessage) -> Result<(), SendError<TB>> {
    if let Some(cell) = self.cell(&pid) {
      cell.dispatcher().enqueue_system(message)
    } else {
      match message {
        | SystemMessage::Watch(watcher) => {
          if self.remote_watch_hook.handle_watch(pid, watcher) {
            return Ok(());
          }
          let _ = self.send_system_message(watcher, SystemMessage::Terminated(pid));
          Ok(())
        },
        | SystemMessage::Unwatch(watcher) => {
          if self.remote_watch_hook.handle_unwatch(pid, watcher) {
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
  pub fn record_send_error(&self, recipient: Option<Pid>, error: &SendError<TB>) {
    let timestamp = self.monotonic_now();
    self.dead_letter.record_send_error(recipient, error, timestamp);
  }

  /// Handles a failure in a child actor according to supervision strategy.
  #[allow(dead_code)]
  pub fn handle_failure(&self, pid: Pid, parent: Option<Pid>, error: &ActorError) {
    let Some(parent_pid) = parent else {
      let _ = self.send_system_message(pid, SystemMessage::Stop);
      return;
    };

    let Some(parent_cell) = self.cell(&parent_pid) else {
      let _ = self.send_system_message(pid, SystemMessage::Stop);
      return;
    };

    let parent_parent = parent_cell.parent();
    let now = self.monotonic_now();
    let (directive, affected) = parent_cell.handle_child_failure(pid, error, now);

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
          let _ = self.send_system_message(target, SystemMessage::Stop);
        }
      },
      | SupervisorDirective::Escalate => {
        for target in affected {
          let _ = self.send_system_message(target, SystemMessage::Stop);
        }
        self.handle_failure(parent_pid, parent_parent, error);
      },
    }
  }

  /// Records an explicit deadletter entry originating from runtime logic.
  pub fn record_dead_letter(&self, message: AnyMessageGeneric<TB>, reason: DeadLetterReason, target: Option<Pid>) {
    let timestamp = self.monotonic_now();
    self.dead_letter.record_entry(message, reason, target, timestamp);
  }

  /// Marks the system as terminated and completes the termination future.
  pub fn mark_terminated(&self) {
    self.inner.read().mark_terminated();
  }

  /// Returns a future that resolves once the actor system terminates.
  #[must_use]
  pub fn termination_future(&self) -> ActorFutureSharedGeneric<(), TB> {
    self.termination.clone()
  }

  /// Drains ask futures that have completed since the previous inspection.
  #[must_use]
  pub fn drain_ready_ask_futures(&self) -> Vec<ActorFutureSharedGeneric<AnyMessageGeneric<TB>, TB>> {
    self.inner.write().drain_ready_ask_futures()
  }

  /// Indicates whether the actor system has terminated.
  #[must_use]
  pub fn is_terminated(&self) -> bool {
    self.inner.read().is_terminated()
  }

  /// Returns a monotonic timestamp for instrumentation.
  #[must_use]
  pub fn monotonic_now(&self) -> Duration {
    self.inner.read().monotonic_now()
  }

  /// Resolves the dispatcher configuration for the identifier.
  ///
  /// # Errors
  ///
  /// Returns [`DispatcherRegistryError::Unknown`] when the identifier has not been registered.
  pub fn resolve_dispatcher(&self, id: &str) -> Result<DispatcherConfigGeneric<TB>, DispatcherRegistryError> {
    self.inner.read().resolve_dispatcher(id)
  }

  /// Resolves the mailbox configuration for the identifier.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Unknown`] when the identifier has not been registered.
  pub fn resolve_mailbox(&self, id: &str) -> Result<MailboxConfig, MailboxRegistryError> {
    self.inner.read().resolve_mailbox(id)
  }

  /// Returns the remoting configuration when it has been configured.
  #[must_use]
  pub fn remoting_config(&self) -> Option<RemotingConfig> {
    self.canonical_host.clone().map(|host| {
      let mut config =
        RemotingConfig::default().with_canonical_host(host).with_quarantine_duration(self.quarantine_duration);
      if let Some(port) = self.canonical_port {
        config = config.with_canonical_port(port);
      }
      config
    })
  }

  /// Returns the scheduler context.
  #[must_use]
  pub fn scheduler_context(&self) -> SchedulerContextSharedGeneric<TB> {
    self.scheduler_ctx.clone()
  }

  /// Returns the tick driver runtime.
  #[must_use]
  pub fn tick_driver_runtime(&self) -> TickDriverRuntime<TB> {
    self.tick_driver_rt.clone()
  }

  /// Returns the last recorded tick driver snapshot when available.
  #[must_use]
  pub fn tick_driver_snapshot(&self) -> Option<TickDriverSnapshot> {
    self.scheduler_ctx.driver_snapshot()
  }

  /// Shuts down the scheduler context if configured.
  #[must_use]
  pub fn shutdown_scheduler(&self) -> Option<TaskRunSummary> {
    Some(self.scheduler_ctx.shutdown())
  }

  /// Records a failure and routes it to the supervising hierarchy.
  pub fn report_failure(&self, mut payload: FailurePayload) {
    {
      self.inner.read().record_failure_reported();
    }

    let child_pid = payload.child();
    let message = format!("actor {:?} failed: {}", child_pid, payload.reason().as_str());
    self.emit_log(LogLevel::Error, message, Some(child_pid));

    if let Some(parent_pid) = self.cell(&child_pid).and_then(|cell| cell.parent())
      && let Some(parent_cell) = self.cell(&parent_pid)
    {
      if let Some(stats) = parent_cell.snapshot_child_restart_stats(child_pid) {
        payload = payload.with_restart_stats(stats);
      }
      if self.send_system_message(parent_pid, SystemMessage::Failure(payload.clone())).is_ok() {
        return;
      }
      self.record_failure_outcome(child_pid, super::FailureOutcome::Stop, &payload);
      let _ = self.send_system_message(child_pid, SystemMessage::Stop);
      return;
    }

    self.record_failure_outcome(child_pid, super::FailureOutcome::Stop, &payload);
    let _ = self.send_system_message(child_pid, SystemMessage::Stop);
  }

  /// Records the outcome of a previously reported failure (restart/stop/escalate).
  pub fn record_failure_outcome(&self, child: Pid, outcome: super::FailureOutcome, payload: &FailurePayload) {
    self.inner.read().record_failure_outcome(child, outcome, payload);
  }

  /// Returns a reference to the ActorPathRegistry.
  pub fn with_actor_path_registry<R, F>(&self, f: F) -> R
  where
    F: FnOnce(&ActorPathRegistry) -> R, {
    let snapshot = { self.inner.read().actor_path_registry().clone() };
    f(&snapshot)
  }

  /// Returns the current authority state.
  #[must_use]
  pub fn remote_authority_state(&self, authority: &str) -> AuthorityState {
    self.inner.read().remote_authority_state(authority)
  }

  /// Returns a snapshot of known remote authorities and their states.
  #[must_use]
  pub fn remote_authority_snapshots(&self) -> Vec<(String, AuthorityState)> {
    self.inner.read().remote_authority_snapshots()
  }

  /// Marks the authority as connected and emits an event.
  #[must_use]
  pub fn remote_authority_set_connected(&self, authority: &str) -> Option<VecDeque<AnyMessageGeneric<TB>>> {
    self.inner.write().remote_authority_set_connected(authority)
  }

  /// Transitions the authority into quarantine.
  pub fn remote_authority_set_quarantine(&self, authority: impl Into<String>, duration: Option<Duration>) {
    self.inner.write().remote_authority_set_quarantine(authority, duration);
  }

  /// Handles an InvalidAssociation signal by moving the authority into quarantine.
  pub fn remote_authority_handle_invalid_association(&self, authority: impl Into<String>, duration: Option<Duration>) {
    self.inner.write().remote_authority_handle_invalid_association(authority, duration);
  }

  /// Manually overrides a quarantined authority back to connected.
  pub fn remote_authority_manual_override_to_connected(&self, authority: &str) {
    self.inner.write().remote_authority_manual_override_to_connected(authority);
  }

  /// Defers a message while the authority is unresolved.
  ///
  /// # Errors
  ///
  /// Returns [`RemoteAuthorityError`] if the authority is quarantined.
  pub fn remote_authority_defer(
    &self,
    authority: impl Into<String>,
    message: AnyMessageGeneric<TB>,
  ) -> Result<(), RemoteAuthorityError> {
    self.inner.write().remote_authority_defer(authority, message)
  }

  /// Attempts to defer a message, returning an error if the authority is quarantined.
  ///
  /// # Errors
  ///
  /// Returns [`RemoteAuthorityError`] if the authority is quarantined.
  pub fn remote_authority_try_defer(
    &self,
    authority: impl Into<String>,
    message: AnyMessageGeneric<TB>,
  ) -> Result<(), RemoteAuthorityError> {
    self.inner.write().remote_authority_try_defer(authority, message)
  }

  /// Polls all authorities for expired quarantine windows.
  pub fn poll_remote_authorities(&self) {
    self.inner.write().poll_remote_authorities();
  }

  /// Returns the number of messages deferred for the provided authority.
  #[must_use]
  pub fn remote_authority_deferred_count(&self, authority: &str) -> usize {
    self.inner.read().remote_authority_deferred_count(authority)
  }
}

/// Type alias with the default `NoStdToolbox`.
pub type SystemStateShared = SystemStateSharedGeneric<NoStdToolbox>;
