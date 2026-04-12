//! Shared wrapper for system state.

#[cfg(test)]
mod tests;

use alloc::{
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

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess, SharedRwLock, SpinSyncRwLock};

use super::{
  ActorPathRegistry, ActorRefProvider, ActorRefProviderHandleShared, AuthorityState, CellsShared, GuardianKind,
  RemoteAuthorityError, RemoteWatchHookDynShared, RemotingConfig, SystemStateWeak,
  system_state::{FailureOutcome, SystemState},
};
use crate::core::kernel::{
  actor::{
    ActorCell, ActorCellStateSharedFactory, ActorSharedLockFactory, Pid, ReceiveTimeoutStateSharedFactory,
    actor_path::{ActorPath, ActorPathParser, ActorPathParts, ActorPathScheme, GuardianKind as PathGuardianKind},
    actor_ref::{
      ActorRef, ActorRefSenderSharedFactory,
      dead_letter::{DeadLetterEntry, DeadLetterReason, DeadLetterShared},
    },
    actor_ref_provider::{ActorRefProviderHandleSharedFactory, LocalActorRefProvider},
    context_pipe::ContextPipeWakerHandleSharedFactory,
    deploy::Deployer,
    error::{ActorError, SendError},
    messaging::{
      AnyMessage, AskResult,
      message_invoker::MessageInvokerSharedFactory,
      system_message::{FailurePayload, SystemMessage},
    },
    props::MailboxConfig,
    scheduler::{
      SchedulerBackedDelayProvider, SchedulerShared, task_run::TaskRunSummary, tick_driver::TickDriverBundle,
    },
    spawn::SpawnError,
    supervision::SupervisorDirective,
  },
  dispatch::{
    dispatcher::MessageDispatcherShared,
    mailbox::{MailboxRegistryError, MessageQueue},
  },
  event::{
    logging::{LogEvent, LogLevel},
    stream::{EventStreamEvent, EventStreamShared, EventStreamSubscriberSharedFactory, TickDriverSnapshot},
  },
  system::{
    ActorSystemBuildError, RegisterExtraTopLevelError, TerminationSignal, shared_factory::MailboxSharedSetFactory,
  },
  util::futures::{ActorFutureShared, ActorFutureSharedFactory},
};

/// Shared wrapper for [`SystemState`] providing thread-safe access.
///
/// This wrapper uses a read-write lock to provide safe concurrent access
/// to the underlying system state.
pub struct SystemStateShared {
  pub(crate) inner:    SharedRwLock<SystemState>,
  system_name:         String,
  guardian_kind:       PathGuardianKind,
  canonical_host:      Option<String>,
  canonical_port:      Option<u16>,
  quarantine_duration: Duration,
  event_stream:        EventStreamShared,
  dead_letter:         DeadLetterShared,
  cells:               CellsShared,
  termination_signal:  TerminationSignal,
  remote_watch_hook:   RemoteWatchHookDynShared,
  scheduler:           SchedulerShared,
  delay_provider:      SchedulerBackedDelayProvider,
  tick_driver_bundle:  TickDriverBundle,
  start_time:          Duration,
}

impl Clone for SystemStateShared {
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
      termination_signal:  self.termination_signal.clone(),
      remote_watch_hook:   self.remote_watch_hook.clone(),
      scheduler:           self.scheduler.clone(),
      delay_provider:      self.delay_provider.clone(),
      tick_driver_bundle:  self.tick_driver_bundle.clone(),
      start_time:          self.start_time,
    }
  }
}

impl SystemStateShared {
  /// Creates a new shared system state.
  #[must_use]
  pub fn new(state: SystemState) -> Self {
    let system_name = state.system_name();
    let guardian_kind = state.path_guardian_kind();
    let canonical_host = state.canonical_host();
    let canonical_port = state.canonical_port();
    let quarantine_duration = state.quarantine_duration();
    let event_stream = state.event_stream();
    let dead_letter = state.dead_letter_store();
    let cells = state.cells_handle();
    let termination_signal = TerminationSignal::new(state.termination_state());
    let remote_watch_hook = state.remote_watch_hook_handle();
    let scheduler = state.scheduler();
    let delay_provider = state.delay_provider();
    let tick_driver_bundle = state.tick_driver_bundle();
    let start_time = state.start_time();
    let inner = SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(state);
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
      termination_signal,
      remote_watch_hook,
      scheduler,
      delay_provider,
      tick_driver_bundle,
      start_time,
    }
  }

  /// Creates a shared wrapper from an existing [`SharedRwLock`].
  #[must_use]
  pub(crate) fn from_shared_rw_lock(inner: SharedRwLock<SystemState>) -> Self {
    let (
      system_name,
      guardian_kind,
      canonical_host,
      canonical_port,
      quarantine_duration,
      event_stream,
      dead_letter,
      cells,
      termination_signal,
      remote_watch_hook,
      scheduler,
      delay_provider,
      tick_driver_bundle,
      start_time,
    ) = inner.with_read(|guard| {
      (
        guard.system_name(),
        guard.path_guardian_kind(),
        guard.canonical_host(),
        guard.canonical_port(),
        guard.quarantine_duration(),
        guard.event_stream(),
        guard.dead_letter_store(),
        guard.cells_handle(),
        TerminationSignal::new(guard.termination_state()),
        guard.remote_watch_hook_handle(),
        guard.scheduler(),
        guard.delay_provider(),
        guard.tick_driver_bundle(),
        guard.start_time(),
      )
    });
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
      termination_signal,
      remote_watch_hook,
      scheduler,
      delay_provider,
      tick_driver_bundle,
      start_time,
    }
  }

  /// Returns the inner reference for direct access when needed.
  #[must_use]
  pub(crate) const fn inner(&self) -> &SharedRwLock<SystemState> {
    &self.inner
  }

  /// Returns the actor-ref-sender shared factory.
  #[must_use]
  pub fn actor_ref_sender_shared_factory(&self) -> ArcShared<dyn ActorRefSenderSharedFactory> {
    self.inner.with_read(|inner| inner.actor_ref_sender_shared_factory())
  }

  /// Returns the actor shared-lock factory.
  #[must_use]
  pub fn actor_shared_lock_factory(&self) -> ArcShared<dyn ActorSharedLockFactory> {
    self.inner.with_read(|inner| inner.actor_shared_lock_factory())
  }

  /// Returns the actor-cell-state shared factory.
  #[must_use]
  pub fn actor_cell_state_shared_factory(&self) -> ArcShared<dyn ActorCellStateSharedFactory> {
    self.inner.with_read(|inner| inner.actor_cell_state_shared_factory())
  }

  /// Returns the receive-timeout-state shared factory.
  #[must_use]
  pub fn receive_timeout_state_shared_factory(&self) -> ArcShared<dyn ReceiveTimeoutStateSharedFactory> {
    self.inner.with_read(|inner| inner.receive_timeout_state_shared_factory())
  }

  /// Returns the message-invoker shared factory.
  #[must_use]
  pub fn message_invoker_shared_factory(&self) -> ArcShared<dyn MessageInvokerSharedFactory> {
    self.inner.with_read(|inner| inner.message_invoker_shared_factory())
  }

  /// Returns the actor-future shared factory.
  #[must_use]
  pub fn actor_future_shared_factory(&self) -> ArcShared<dyn ActorFutureSharedFactory<AskResult>> {
    self.inner.with_read(|inner| inner.actor_future_shared_factory())
  }

  /// Returns the local actor-ref-provider handle shared factory.
  #[must_use]
  pub fn local_actor_ref_provider_handle_shared_factory(
    &self,
  ) -> ArcShared<dyn ActorRefProviderHandleSharedFactory<LocalActorRefProvider>> {
    self.inner.with_read(|inner| inner.local_actor_ref_provider_handle_shared_factory())
  }

  /// Returns the event-stream-subscriber shared factory.
  #[must_use]
  pub fn event_stream_subscriber_shared_factory(&self) -> ArcShared<dyn EventStreamSubscriberSharedFactory> {
    self.inner.with_read(|inner| inner.event_stream_subscriber_shared_factory())
  }

  /// Returns the mailbox-shared-set factory.
  #[must_use]
  pub fn mailbox_shared_set_factory(&self) -> ArcShared<dyn MailboxSharedSetFactory> {
    self.inner.with_read(|inner| inner.mailbox_shared_set_factory())
  }

  /// Returns the context-pipe-waker-handle shared factory.
  #[must_use]
  pub fn context_pipe_waker_handle_shared_factory(&self) -> ArcShared<dyn ContextPipeWakerHandleSharedFactory> {
    self.inner.with_read(|inner| inner.context_pipe_waker_handle_shared_factory())
  }

  /// Creates a weak reference to this system state.
  #[must_use]
  pub fn downgrade(&self) -> SystemStateWeak {
    SystemStateWeak { inner: self.inner.downgrade() }
  }

  // ====== SystemState の委譲メソッド ======

  /// Allocates a new unique [`Pid`] for an actor.
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    self.inner.with_read(|inner| inner.allocate_pid())
  }

  /// Registers the provided actor cell in the global registry.
  pub fn register_cell(&self, cell: ArcShared<ActorCell>) {
    let pid = cell.pid();
    self.cells.with_write(|cells| cells.insert(pid, cell));
    if let Some(path) = self.canonical_actor_path(&pid) {
      self.inner.with_write(|inner| inner.actor_path_registry_mut().register(pid, &path));
    }
  }

  /// Removes the actor cell associated with the pid.
  pub fn remove_cell(&self, pid: &Pid) {
    // Detach from the new dispatcher tree before destroying the cell, so
    // inhabitants tracking stays balanced.
    if let Some(cell) = self.cell(pid) {
      let new_dispatcher = cell.new_dispatcher_shared();
      let _schedule = new_dispatcher.detach(&cell);
    }

    let reservation_source = self.inner.with_read(|inner| {
      inner.actor_path_registry().get(pid).map(|handle| (handle.canonical_uri().to_string(), handle.uid()))
    });

    if let Some((canonical, Some(uid))) = reservation_source
      && let Ok(actor_path) = ActorPathParser::parse(&canonical)
    {
      let now_secs = self.monotonic_now().as_secs();
      let reserve_result = self.inner.with_write(|guard| {
        let reserve_result = guard.actor_path_registry_mut().reserve_uid(&actor_path, uid, now_secs, None);
        guard.actor_path_registry_mut().unregister(pid);
        reserve_result
      });
      if let Err(e) = reserve_result {
        self.emit_log(LogLevel::Warn, format!("failed to reserve uid for {:?}: {:?}", pid, e), Some(*pid), None);
      }
      // Intentionally discarding the removed cell; this is a HashMap::remove equivalent.
      drop(self.cells.with_write(|cells| cells.remove(pid)));
      return;
    }

    self.inner.with_write(|inner| inner.actor_path_registry_mut().unregister(pid));
    // Intentionally discarding the removed cell; this is a HashMap::remove equivalent.
    drop(self.cells.with_write(|cells| cells.remove(pid)));
  }

  /// Returns the canonical actor path for the given pid when available.
  #[must_use]
  pub fn canonical_actor_path(&self, pid: &Pid) -> Option<ActorPath> {
    let base = self.actor_path(pid)?;
    let segments = base.segments().to_vec();
    let parts = self.canonical_parts()?;
    Some(ActorPath::from_parts_and_segments(parts, segments, base.uid()))
  }

  /// Registers a canonical actor path for a synthetic pid.
  pub fn register_actor_path(&self, pid: Pid, path: &ActorPath) {
    self.inner.with_write(|inner| inner.register_actor_path(pid, path));
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

  /// Returns a snapshot of the deployer registry.
  #[must_use]
  pub fn deployer(&self) -> Deployer {
    self.inner.with_read(|inner| inner.deployer())
  }

  /// Returns the start time of the actor system (epoch-relative duration).
  ///
  /// Corresponds to Pekko's `ActorSystem.startTime`.
  #[must_use]
  pub const fn start_time(&self) -> Duration {
    self.start_time
  }

  /// Retrieves an actor cell by pid.
  #[must_use]
  pub fn cell(&self, pid: &Pid) -> Option<ArcShared<ActorCell>> {
    self.cells.with_read(|cells| cells.get(pid))
  }

  /// Binds an actor name within its parent's scope.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] if the name assignment fails.
  pub fn assign_name(&self, parent: Option<Pid>, hint: Option<&str>, pid: Pid) -> Result<String, SpawnError> {
    self.inner.with_write(|inner| inner.assign_name(parent, hint, pid))
  }

  /// Releases the association between a name and its pid in the registry.
  pub fn release_name(&self, parent: Option<Pid>, name: &str) {
    self.inner.with_write(|inner| inner.release_name(parent, name));
  }

  /// Stores the root guardian cell reference.
  pub(crate) fn set_root_guardian(&self, cell: &ArcShared<ActorCell>) {
    self.inner.with_write(|inner| inner.set_root_guardian(cell));
  }

  /// Stores the system guardian cell reference.
  pub(crate) fn set_system_guardian(&self, cell: &ArcShared<ActorCell>) {
    self.inner.with_write(|inner| inner.set_system_guardian(cell));
  }

  /// Stores the user guardian cell reference.
  pub(crate) fn set_user_guardian(&self, cell: &ArcShared<ActorCell>) {
    self.inner.with_write(|inner| inner.set_user_guardian(cell));
  }

  /// Returns the guardian kind matching the provided pid when registered.
  #[must_use]
  pub fn guardian_kind_by_pid(&self, pid: Pid) -> Option<GuardianKind> {
    self.inner.with_read(|inner| inner.guardian_kind_by_pid(pid))
  }

  /// Marks the specified guardian as stopped.
  pub fn mark_guardian_stopped(&self, kind: GuardianKind) {
    self.inner.with_read(|inner| inner.mark_guardian_stopped(kind));
  }

  /// Returns the root guardian cell if initialised.
  #[must_use]
  pub fn root_guardian(&self) -> Option<ArcShared<ActorCell>> {
    self.inner.with_read(|inner| inner.root_guardian())
  }

  /// Returns the system guardian cell if initialised.
  #[must_use]
  pub fn system_guardian(&self) -> Option<ArcShared<ActorCell>> {
    self.inner.with_read(|inner| inner.system_guardian())
  }

  /// Returns the user guardian cell if initialised.
  #[must_use]
  pub fn user_guardian(&self) -> Option<ArcShared<ActorCell>> {
    self.inner.with_read(|inner| inner.user_guardian())
  }

  /// Returns the pid of the root guardian if available.
  #[must_use]
  pub fn root_guardian_pid(&self) -> Option<Pid> {
    self.inner.with_read(|inner| inner.root_guardian_pid())
  }

  /// Returns the pid of the system guardian if available.
  #[must_use]
  pub fn system_guardian_pid(&self) -> Option<Pid> {
    self.inner.with_read(|inner| inner.system_guardian_pid())
  }

  /// Returns the pid of the user guardian if available.
  #[must_use]
  pub fn user_guardian_pid(&self) -> Option<Pid> {
    self.inner.with_read(|inner| inner.user_guardian_pid())
  }

  /// Returns the PID registered for the specified guardian.
  #[must_use]
  pub fn guardian_pid(&self, kind: GuardianKind) -> Option<Pid> {
    self.inner.with_read(|inner| inner.guardian_pid(kind))
  }

  /// Registers a PID for the specified guardian kind.
  #[cfg(any(test, feature = "test-support"))]
  pub(crate) fn register_guardian_pid(&self, kind: GuardianKind, pid: Pid) {
    self.inner.with_write(|inner| inner.register_guardian_pid(kind, pid));
  }

  /// Returns whether the specified guardian is alive.
  #[must_use]
  pub fn guardian_alive(&self, kind: GuardianKind) -> bool {
    self.inner.with_read(|inner| inner.guardian_alive(kind))
  }

  /// Registers an extra top-level path prior to root startup.
  ///
  /// # Errors
  ///
  /// Returns [`RegisterExtraTopLevelError`] if the registration fails.
  pub fn register_extra_top_level(&self, name: &str, actor: ActorRef) -> Result<(), RegisterExtraTopLevelError> {
    if self.inner.with_read(|inner| inner.has_root_started()) {
      return Err(RegisterExtraTopLevelError::AlreadyStarted);
    }
    self.inner.with_write(|inner| inner.register_extra_top_level(name, actor))
  }

  /// Returns a registered extra top-level reference if present.
  #[must_use]
  pub fn extra_top_level(&self, name: &str) -> Option<ActorRef> {
    self.inner.with_read(|inner| inner.extra_top_level(name))
  }

  /// Marks the root guardian as fully initialised.
  pub fn mark_root_started(&self) {
    self.inner.with_read(|inner| inner.mark_root_started());
  }

  /// Indicates whether the root guardian has completed startup.
  #[must_use]
  pub fn has_root_started(&self) -> bool {
    self.inner.with_read(|inner| inner.has_root_started())
  }

  /// Attempts to transition the system into the terminating state.
  #[must_use]
  pub fn begin_termination(&self) -> bool {
    self.inner.with_read(|inner| inner.begin_termination())
  }

  /// Indicates whether the system is currently terminating.
  #[must_use]
  pub fn is_terminating(&self) -> bool {
    self.inner.with_read(|inner| inner.is_terminating())
  }

  /// Generates a unique `/temp` path segment and registers the supplied actor reference.
  #[must_use]
  pub fn register_temp_actor(&self, actor: ActorRef) -> String {
    self.inner.with_write(|inner| inner.register_temp_actor(actor))
  }

  /// Generates a unique `/temp` path segment using the provided prefix hint.
  #[must_use]
  pub fn next_temp_actor_name_with_prefix(&self, prefix: &str) -> String {
    self.inner.with_write(|inner| inner.next_temp_actor_name_with_prefix(prefix))
  }

  /// Removes a temporary actor reference if registered.
  pub fn unregister_temp_actor(&self, name: &str) {
    self.inner.with_write(|inner| inner.unregister_temp_actor(name));
  }

  /// Unregisters a temporary actor by pid when present.
  pub fn unregister_temp_actor_by_pid(&self, pid: &Pid) {
    self.inner.with_write(|inner| inner.unregister_temp_actor_by_pid(pid));
  }

  /// Resolves a registered temporary actor reference.
  #[must_use]
  pub fn temp_actor(&self, name: &str) -> Option<ActorRef> {
    self.inner.with_read(|inner| inner.temp_actor(name))
  }

  /// Resolves the actor path for the specified pid if the actor exists.
  #[must_use]
  pub fn actor_path(&self, pid: &Pid) -> Option<ActorPath> {
    let Some(cell) = self.cell(pid) else {
      let canonical =
        self.inner.with_read(|guard| guard.actor_path_registry().canonical_uri(pid).map(|value| value.to_string()))?;
      return ActorPathParser::parse(&canonical).ok();
    };
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
    let (guardian_kind, actor_segments) = match segments.first().map(String::as_str) {
      | Some("system") => (PathGuardianKind::System, &segments[1..]),
      | Some("user") => (PathGuardianKind::User, &segments[1..]),
      | _ => (self.guardian_kind, segments.as_slice()),
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

  /// Registers an ask future so the actor system can track its completion.
  pub fn register_ask_future(&self, future: ActorFutureShared<AskResult>) {
    self.inner.with_write(|inner| inner.register_ask_future(future));
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

  /// Returns `true` when an extension for the provided [`TypeId`] is registered.
  #[must_use]
  pub fn has_extension(&self, type_id: TypeId) -> bool {
    self.inner.with_read(|inner| inner.has_extension(type_id))
  }

  /// Returns an extension by [`TypeId`].
  #[must_use]
  pub fn extension<E>(&self, type_id: TypeId) -> Option<ArcShared<E>>
  where
    E: Any + Send + Sync + 'static, {
    self.inner.with_read(|inner| inner.extension(type_id))
  }

  /// Inserts an extension if absent and returns the shared instance.
  ///
  /// Registers an extension or returns the existing one (putIfAbsent semantics).
  ///
  /// # Panics
  ///
  /// Panics when an extension exists for `type_id` but cannot be downcast to `E`.
  pub fn extension_or_insert_with<E, F>(&self, type_id: TypeId, factory: F) -> ArcShared<E>
  where
    E: Any + Send + Sync + 'static,
    F: FnOnce() -> ArcShared<E>, {
    if let Some(existing) = self.inner.with_read(|guard| guard.extension_raw(&type_id)) {
      if let Ok(extension) = existing.downcast::<E>() {
        return extension;
      }
      panic!("extension type mismatch for id {type_id:?}");
    }

    let created = factory();
    let erased: ArcShared<dyn Any + Send + Sync + 'static> = created.clone();

    self.inner.with_write(|guard| {
      if let Some(existing) = guard.extension_raw(&type_id) {
        if let Ok(extension) = existing.downcast::<E>() {
          return extension;
        }
        panic!("extension type mismatch for id {type_id:?}");
      }
      guard.insert_extension(type_id, erased);
      created.clone()
    })
  }

  /// Returns an extension by its type.
  #[must_use]
  pub fn extension_by_type<E>(&self) -> Option<ArcShared<E>>
  where
    E: Any + Send + Sync + 'static, {
    self.inner.with_read(|inner| inner.extension_by_type())
  }

  /// Installs an actor ref provider.
  ///
  /// # Errors
  ///
  /// Returns [`ActorSystemBuildError::Configuration`] when called after system startup.
  pub fn install_actor_ref_provider<P>(
    &self,
    provider: &ActorRefProviderHandleShared<P>,
  ) -> Result<(), ActorSystemBuildError>
  where
    P: ActorRefProvider + Any + Send + Sync + 'static, {
    self.inner.with_write(|guard| {
      if guard.has_root_started() {
        return Err(ActorSystemBuildError::Configuration(
          "actor-ref provider registration is only allowed before system startup".into(),
        ));
      }
      guard.install_actor_ref_provider(provider);
      Ok(())
    })
  }

  /// Registers a remote watch hook.
  pub fn register_remote_watch_hook(&self, hook: Box<dyn super::RemoteWatchHook>) {
    self.remote_watch_hook.replace(hook);
  }

  /// Returns an actor ref provider.
  #[must_use]
  pub fn actor_ref_provider<P>(&self) -> Option<ActorRefProviderHandleShared<P>>
  where
    P: ActorRefProvider + Any + Send + Sync + 'static, {
    self.inner.with_read(|inner| inner.actor_ref_provider())
  }

  /// Invokes a provider registered for the given scheme.
  #[must_use]
  pub fn actor_ref_provider_call_for_scheme(
    &self,
    scheme: ActorPathScheme,
    path: ActorPath,
  ) -> Option<Result<ActorRef, ActorError>> {
    let caller = self.inner.with_read(|guard| guard.actor_ref_provider_caller_for_scheme(scheme))?;
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
  pub fn send_system_message(&self, pid: Pid, message: SystemMessage) -> Result<(), SendError> {
    if let Some(cell) = self.cell(&pid) {
      cell.new_dispatcher_shared().system_dispatch(&cell, message)
    } else {
      match message {
        | SystemMessage::Watch(watcher) => {
          if self.remote_watch_hook.handle_watch(pid, watcher) {
            return Ok(());
          }
          if let Err(e) = self.send_system_message(watcher, SystemMessage::Terminated(pid)) {
            self.record_send_error(Some(watcher), &e);
          }
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
        | other => Err(SendError::closed(AnyMessage::new(other))),
      }
    }
  }

  /// Records a send error for diagnostics.
  pub fn record_send_error(&self, recipient: Option<Pid>, error: &SendError) {
    let timestamp = self.monotonic_now();
    self.dead_letter.record_send_error(recipient, error, timestamp);
  }

  /// Handles a failure in a child actor according to supervision strategy.
  #[allow(dead_code)]
  pub fn handle_failure(&self, pid: Pid, parent: Option<Pid>, error: &ActorError) {
    let Some(parent_pid) = parent else {
      if let Err(e) = self.send_system_message(pid, SystemMessage::Stop) {
        self.record_send_error(Some(pid), &e);
      }
      return;
    };

    let Some(parent_cell) = self.cell(&parent_pid) else {
      if let Err(e) = self.send_system_message(pid, SystemMessage::Stop) {
        self.record_send_error(Some(pid), &e);
      }
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
          if let Err(e) = self.send_system_message(target, SystemMessage::Stop) {
            self.record_send_error(Some(target), &e);
          }
        }
      },
      | SupervisorDirective::Escalate => {
        for target in affected {
          if let Err(e) = self.send_system_message(target, SystemMessage::Stop) {
            self.record_send_error(Some(target), &e);
          }
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

  /// Records an explicit deadletter entry originating from runtime logic.
  pub fn record_dead_letter(&self, message: AnyMessage, reason: DeadLetterReason, target: Option<Pid>) {
    let timestamp = self.monotonic_now();
    self.dead_letter.record_entry(message, reason, target, timestamp);
  }

  /// Marks the system as terminated and completes the termination future.
  pub fn mark_terminated(&self) {
    self.inner.with_read(|inner| inner.mark_terminated());
  }

  /// Returns a signal that resolves once the actor system terminates.
  #[must_use]
  pub fn termination_signal(&self) -> TerminationSignal {
    self.termination_signal.clone()
  }

  /// Drains ask futures that have completed since the previous inspection.
  #[must_use]
  pub fn drain_ready_ask_futures(&self) -> Vec<ActorFutureShared<AskResult>> {
    self.inner.with_write(|inner| inner.drain_ready_ask_futures())
  }

  /// Indicates whether the actor system has terminated.
  #[must_use]
  pub fn is_terminated(&self) -> bool {
    self.inner.with_read(|inner| inner.is_terminated())
  }

  /// Returns a monotonic timestamp for instrumentation.
  #[must_use]
  pub fn monotonic_now(&self) -> Duration {
    self.inner.with_read(|inner| inner.monotonic_now())
  }

  /// Resolves a [`MessageDispatcherShared`] for the identifier.
  ///
  /// Returns `None` when no configurator is registered for the id.
  #[must_use]
  pub fn resolve_dispatcher(&self, id: &str) -> Option<MessageDispatcherShared> {
    self.inner.with_read(|inner| inner.resolve_dispatcher(id))
  }

  /// Returns the cumulative number of `Dispatchers::resolve` invocations
  /// observed by the actor system's dispatcher registry.
  ///
  /// Diagnostics-only accessor used by integration tests to verify the
  /// call-frequency contract.
  #[must_use]
  pub fn dispatcher_resolve_call_count(&self) -> usize {
    self.inner.with_read(|inner| inner.dispatcher_resolve_call_count())
  }

  /// Resolves the mailbox configuration for the identifier.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Unknown`] when the identifier has not been registered.
  pub fn resolve_mailbox(&self, id: &str) -> Result<MailboxConfig, MailboxRegistryError> {
    self.inner.with_read(|inner| inner.resolve_mailbox(id))
  }

  /// Creates a mailbox queue from the configuration registered under the identifier.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Unknown`] when the identifier has not been registered.
  pub fn create_mailbox_queue(&self, id: &str) -> Result<Box<dyn MessageQueue>, MailboxRegistryError> {
    self.inner.with_read(|inner| inner.create_mailbox_queue(id))
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

  /// Returns the shared scheduler handle.
  #[must_use]
  pub fn scheduler(&self) -> SchedulerShared {
    self.scheduler.clone()
  }

  /// Returns the delay provider connected to the scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider {
    self.delay_provider.clone()
  }

  /// Returns the tick driver bundle.
  #[must_use]
  pub fn tick_driver_bundle(&self) -> TickDriverBundle {
    self.tick_driver_bundle.clone()
  }

  /// Returns the last recorded tick driver snapshot when available.
  #[must_use]
  pub fn tick_driver_snapshot(&self) -> Option<TickDriverSnapshot> {
    self.inner.with_read(|inner| inner.tick_driver_snapshot())
  }

  /// Shuts down the scheduler context if configured.
  #[must_use]
  pub fn shutdown_scheduler(&self) -> Option<TaskRunSummary> {
    let scheduler = self.scheduler();
    Some(scheduler.with_write(|s| s.shutdown_with_tasks()))
  }

  /// Records a failure and routes it to the supervising hierarchy.
  pub fn report_failure(&self, mut payload: FailurePayload) {
    self.inner.with_read(|inner| inner.record_failure_reported());

    let child_pid = payload.child();

    if let Some(parent_pid) = self.cell(&child_pid).and_then(|cell| cell.parent())
      && let Some(parent_cell) = self.cell(&parent_pid)
    {
      if let Some(stats) = parent_cell.snapshot_child_restart_stats(child_pid) {
        payload = payload.with_restart_stats(stats);
      }
      // Avoid reading the parent actor's supervisor strategy here. Failures can be
      // reported while the parent still holds its actor lock during a synchronous
      // forward, and the parent will apply its strategy when it handles Failure.
      if self.send_system_message(parent_pid, SystemMessage::Failure(payload.clone())).is_ok() {
        return;
      }
      self.record_failure_outcome(child_pid, FailureOutcome::Stop, &payload);
      if let Err(e) = self.send_system_message(child_pid, SystemMessage::Stop) {
        self.record_send_error(Some(child_pid), &e);
      }
      return;
    }

    let message = format!("actor {:?} failed: {}", child_pid, payload.reason().as_str());
    self.emit_log(LogLevel::Error, message, Some(child_pid), None);
    self.record_failure_outcome(child_pid, FailureOutcome::Stop, &payload);
    if let Err(e) = self.send_system_message(child_pid, SystemMessage::Stop) {
      self.record_send_error(Some(child_pid), &e);
    }
  }

  /// Records the outcome of a previously reported failure (restart/stop/escalate).
  pub fn record_failure_outcome(&self, child: Pid, outcome: FailureOutcome, payload: &FailurePayload) {
    self.inner.with_read(|inner| inner.record_failure_outcome(child, outcome, payload));
  }

  /// Returns a reference to the ActorPathRegistry.
  pub fn with_actor_path_registry<R, F>(&self, f: F) -> R
  where
    F: FnOnce(&ActorPathRegistry) -> R, {
    let snapshot = self.inner.with_read(|inner| inner.actor_path_registry().clone());
    f(&snapshot)
  }

  /// Returns the current authority state.
  #[must_use]
  pub fn remote_authority_state(&self, authority: &str) -> AuthorityState {
    self.inner.with_read(|inner| inner.remote_authority_state(authority))
  }

  /// Returns a snapshot of known remote authorities and their states.
  #[must_use]
  pub fn remote_authority_snapshots(&self) -> Vec<(String, AuthorityState)> {
    self.inner.with_read(|inner| inner.remote_authority_snapshots())
  }

  /// Marks the authority as connected and emits an event.
  #[must_use]
  pub fn remote_authority_set_connected(&self, authority: &str) -> Option<VecDeque<AnyMessage>> {
    self.inner.with_write(|inner| inner.remote_authority_set_connected(authority))
  }

  /// Transitions the authority into quarantine.
  pub fn remote_authority_set_quarantine(&self, authority: impl Into<String>, duration: Option<Duration>) {
    self.inner.with_write(|inner| inner.remote_authority_set_quarantine(authority, duration));
  }

  /// Handles an InvalidAssociation signal by moving the authority into quarantine.
  pub fn remote_authority_handle_invalid_association(&self, authority: impl Into<String>, duration: Option<Duration>) {
    self.inner.with_write(|inner| inner.remote_authority_handle_invalid_association(authority, duration));
  }

  /// Manually overrides a quarantined authority back to connected.
  pub fn remote_authority_manual_override_to_connected(&self, authority: &str) {
    self.inner.with_write(|inner| inner.remote_authority_manual_override_to_connected(authority));
  }

  /// Defers a message while the authority is unresolved.
  ///
  /// # Errors
  ///
  /// Returns [`RemoteAuthorityError`] if the authority is quarantined.
  pub fn remote_authority_defer(
    &self,
    authority: impl Into<String>,
    message: AnyMessage,
  ) -> Result<(), RemoteAuthorityError> {
    self.inner.with_write(|inner| inner.remote_authority_defer(authority, message))
  }

  /// Attempts to defer a message, returning an error if the authority is quarantined.
  ///
  /// # Errors
  ///
  /// Returns [`RemoteAuthorityError`] if the authority is quarantined.
  pub fn remote_authority_try_defer(
    &self,
    authority: impl Into<String>,
    message: AnyMessage,
  ) -> Result<(), RemoteAuthorityError> {
    self.inner.with_write(|inner| inner.remote_authority_try_defer(authority, message))
  }

  /// Polls all authorities for expired quarantine windows.
  pub fn poll_remote_authorities(&self) {
    self.inner.with_write(|inner| inner.poll_remote_authorities());
  }

  /// Returns the number of messages deferred for the provided authority.
  #[must_use]
  pub fn remote_authority_deferred_count(&self, authority: &str) -> usize {
    self.inner.with_read(|inner| inner.remote_authority_deferred_count(authority))
  }
}
