//! Shared wrapper for system state.

#[cfg(test)]
mod tests;

use alloc::{collections::VecDeque, string::String, vec::Vec};
use core::{any::TypeId, time::Duration};

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncRwLockFamily, ToolboxRwLock},
  sync::{ArcShared, SharedAccess, sync_rwlock_like::SyncRwLockLike},
};

use super::{
  ActorPathRegistrySharedGeneric, ActorRefProvider, ActorRefProviderSharedGeneric, AuthorityState, GuardianKind,
  RemoteAuthorityError, RemoteAuthorityManagerSharedGeneric, RemotingConfig, SystemStateGeneric,
};
use crate::core::{
  actor_prim::{
    ActorCellGeneric, Pid,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::ActorRefGeneric,
  },
  dead_letter::{DeadLetterEntryGeneric, DeadLetterReason, DeadLetterSharedGeneric},
  dispatcher::DispatchersSharedGeneric,
  error::{ActorError, SendError},
  event_stream::{EventStreamEvent, EventStreamSharedGeneric, TickDriverSnapshot},
  futures::ActorFutureSharedGeneric,
  logging::LogLevel,
  mailbox::MailboxesSharedGeneric,
  messaging::{AnyMessageGeneric, FailurePayload, SystemMessage},
  scheduler::{SchedulerContextSharedGeneric, TaskRunSummary, TickDriverRuntime},
  spawn::SpawnError,
  system::{ActorSystemConfigGeneric, RegisterExtensionError, RegisterExtraTopLevelError},
};

/// Shared wrapper for [`SystemStateGeneric`] providing thread-safe access.
///
/// This wrapper uses a read-write lock to provide safe concurrent access
/// to the underlying system state.
pub struct SystemStateSharedGeneric<TB: RuntimeToolbox + 'static> {
  pub(crate) inner: ArcShared<ToolboxRwLock<SystemStateGeneric<TB>, TB>>,
  event_stream:     EventStreamSharedGeneric<TB>,
  dead_letter:      DeadLetterSharedGeneric<TB>,
  dispatchers:      DispatchersSharedGeneric<TB>,
  mailboxes:        MailboxesSharedGeneric<TB>,
  scheduler_ctx:    SchedulerContextSharedGeneric<TB>,
  tick_driver_rt:   TickDriverRuntime<TB>,
  path_registry:    ActorPathRegistrySharedGeneric<TB>,
  remote_mgr:       RemoteAuthorityManagerSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> Clone for SystemStateSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self {
      inner:          self.inner.clone(),
      event_stream:   self.event_stream.clone(),
      dead_letter:    self.dead_letter.clone(),
      dispatchers:    self.dispatchers.clone(),
      mailboxes:      self.mailboxes.clone(),
      scheduler_ctx:  self.scheduler_ctx.clone(),
      tick_driver_rt: self.tick_driver_rt.clone(),
      path_registry:  self.path_registry.clone(),
      remote_mgr:     self.remote_mgr.clone(),
    }
  }
}

impl<TB: RuntimeToolbox + 'static> SystemStateSharedGeneric<TB> {
  /// Creates a new shared system state.
  #[must_use]
  pub fn new(state: SystemStateGeneric<TB>) -> Self {
    let event_stream = state.event_stream();
    let dead_letter = state.dead_letter_store();
    let dispatchers = state.dispatchers();
    let mailboxes = state.mailboxes();
    let scheduler_ctx = state.scheduler_context();
    let tick_driver_rt = state.tick_driver_runtime();
    let path_registry = state.actor_path_registry().clone();
    let remote_mgr = state.remote_authority_manager().clone();
    let inner = ArcShared::new(<TB::RwLockFamily as SyncRwLockFamily>::create(state));
    Self {
      inner,
      event_stream,
      dead_letter,
      dispatchers,
      mailboxes,
      scheduler_ctx,
      tick_driver_rt,
      path_registry,
      remote_mgr,
    }
  }

  /// Creates a shared wrapper from an existing [`ArcShared`].
  #[must_use]
  pub(crate) fn from_arc_shared(inner: ArcShared<ToolboxRwLock<SystemStateGeneric<TB>, TB>>) -> Self {
    let guard = inner.read();
    let event_stream = guard.event_stream();
    let dead_letter = guard.dead_letter_store();
    let dispatchers = guard.dispatchers();
    let mailboxes = guard.mailboxes();
    let scheduler_ctx = guard.scheduler_context();
    let tick_driver_rt = guard.tick_driver_runtime();
    let path_registry = guard.actor_path_registry().clone();
    let remote_mgr = guard.remote_authority_manager().clone();
    drop(guard);
    Self {
      inner,
      event_stream,
      dead_letter,
      dispatchers,
      mailboxes,
      scheduler_ctx,
      tick_driver_rt,
      path_registry,
      remote_mgr,
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

  // ====== Delegated methods from SystemStateGeneric ======

  /// Allocates a new unique [`Pid`] for an actor.
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    self.inner.read().allocate_pid()
  }

  /// Applies the actor system configuration.
  pub fn apply_actor_system_config(&self, config: &ActorSystemConfigGeneric<TB>) {
    self.inner.write().apply_actor_system_config(config);
  }

  /// Registers the provided actor cell in the global registry.
  pub fn register_cell(&self, cell: ArcShared<ActorCellGeneric<TB>>) {
    self.inner.read().register_cell(cell);
  }

  /// Removes the actor cell associated with the pid.
  #[must_use]
  pub fn remove_cell(&self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.inner.read().remove_cell(pid)
  }

  /// Returns the canonical actor path for the given pid when available.
  #[must_use]
  pub fn canonical_actor_path(&self, pid: &Pid) -> Option<ActorPath> {
    self.inner.read().canonical_actor_path(pid)
  }

  /// Returns the configured canonical host/port pair when remoting is enabled.
  #[must_use]
  pub fn canonical_authority_components(&self) -> Option<(String, Option<u16>)> {
    self.inner.read().canonical_authority_components()
  }

  /// Returns true when canonical_host is set but canonical_port is missing.
  #[must_use]
  pub fn has_partial_canonical_authority(&self) -> bool {
    self.inner.read().has_partial_canonical_authority()
  }

  /// Returns the canonical authority string.
  #[must_use]
  pub fn canonical_authority_endpoint(&self) -> Option<String> {
    self.inner.read().canonical_authority_endpoint()
  }

  /// Returns the configured actor system name.
  #[must_use]
  pub fn system_name(&self) -> String {
    self.inner.read().system_name()
  }

  /// Retrieves an actor cell by pid.
  #[must_use]
  pub fn cell(&self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.inner.read().cell(pid)
  }

  /// Binds an actor name within its parent's scope.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] if the name assignment fails.
  pub fn assign_name(&self, parent: Option<Pid>, hint: Option<&str>, pid: Pid) -> Result<String, SpawnError> {
    self.inner.read().assign_name(parent, hint, pid)
  }

  /// Releases the association between a name and its pid in the registry.
  pub fn release_name(&self, parent: Option<Pid>, name: &str) {
    self.inner.read().release_name(parent, name);
  }

  /// Stores the root guardian cell reference.
  pub fn set_root_guardian(&self, cell: &ArcShared<ActorCellGeneric<TB>>) {
    self.inner.read().set_root_guardian(cell);
  }

  /// Stores the system guardian cell reference.
  pub fn set_system_guardian(&self, cell: &ArcShared<ActorCellGeneric<TB>>) {
    self.inner.read().set_system_guardian(cell);
  }

  /// Stores the user guardian cell reference.
  pub fn set_user_guardian(&self, cell: &ArcShared<ActorCellGeneric<TB>>) {
    self.inner.read().set_user_guardian(cell);
  }

  /// Clears the guardian slot matching the pid and returns which guardian stopped.
  #[must_use]
  pub fn clear_guardian(&self, pid: Pid) -> Option<GuardianKind> {
    self.inner.read().clear_guardian(pid)
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
  pub fn register_guardian_pid(&self, kind: GuardianKind, pid: Pid) {
    self.inner.read().register_guardian_pid(kind, pid);
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
    self.inner.read().register_temp_actor(actor)
  }

  /// Removes a temporary actor reference if registered.
  #[must_use]
  pub fn unregister_temp_actor(&self, name: &str) -> Option<ActorRefGeneric<TB>> {
    self.inner.read().unregister_temp_actor(name)
  }

  /// Resolves a registered temporary actor reference.
  #[must_use]
  pub fn temp_actor(&self, name: &str) -> Option<ActorRefGeneric<TB>> {
    self.inner.read().temp_actor(name)
  }

  /// Resolves the actor path for the specified pid if the actor exists.
  #[must_use]
  pub fn actor_path(&self, pid: &Pid) -> Option<ActorPath> {
    self.inner.read().actor_path(pid)
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
    self.inner.read().register_ask_future(future);
  }

  /// Publishes an event to all event stream subscribers.
  pub fn publish_event(&self, event: &EventStreamEvent<TB>) {
    self.event_stream.publish(event);
  }

  /// Emits a log event via the event stream.
  pub fn emit_log(&self, level: LogLevel, message: String, origin: Option<Pid>) {
    self.inner.read().emit_log(level, message, origin);
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
  pub fn install_actor_ref_provider<P>(&self, provider: &ActorRefProviderSharedGeneric<TB, P>)
  where
    P: ActorRefProvider<TB> + core::any::Any + Send + Sync + 'static, {
    self.inner.read().install_actor_ref_provider(provider);
  }

  /// Registers a remote watch hook.
  pub fn register_remote_watch_hook(&self, hook: alloc::boxed::Box<dyn super::RemoteWatchHook<TB>>) {
    self.inner.read().register_remote_watch_hook(hook);
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
    self.inner.read().actor_ref_provider_call_for_scheme(scheme, path)
  }

  /// Registers a child under the specified parent pid.
  pub fn register_child(&self, parent: Pid, child: Pid) {
    self.inner.read().register_child(parent, child);
  }

  /// Removes a child from its parent's supervision registry.
  pub fn unregister_child(&self, parent: Option<Pid>, child: Pid) {
    self.inner.read().unregister_child(parent, child);
  }

  /// Returns the children supervised by the specified parent pid.
  #[must_use]
  pub fn child_pids(&self, parent: Pid) -> Vec<Pid> {
    self.inner.read().child_pids(parent)
  }

  /// Sends a system message to the specified actor.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] if the message cannot be delivered.
  pub fn send_system_message(&self, pid: Pid, message: SystemMessage) -> Result<(), SendError<TB>> {
    self.inner.read().send_system_message(pid, message)
  }

  /// Records a send error for diagnostics.
  pub fn record_send_error(&self, recipient: Option<Pid>, error: &SendError<TB>) {
    self.inner.read().record_send_error(recipient, error);
  }

  /// Handles a failure in a child actor according to supervision strategy.
  #[allow(dead_code)]
  pub fn handle_failure(&self, pid: Pid, parent: Option<Pid>, error: &ActorError) {
    self.inner.read().handle_failure(pid, parent, error);
  }

  /// Records an explicit deadletter entry originating from runtime logic.
  pub fn record_dead_letter(&self, message: AnyMessageGeneric<TB>, reason: DeadLetterReason, target: Option<Pid>) {
    self.inner.read().record_dead_letter(message, reason, target);
  }

  /// Marks the system as terminated and completes the termination future.
  pub fn mark_terminated(&self) {
    self.inner.read().mark_terminated();
  }

  /// Returns a future that resolves once the actor system terminates.
  #[must_use]
  pub fn termination_future(&self) -> ActorFutureSharedGeneric<(), TB> {
    self.inner.read().termination_future()
  }

  /// Drains ask futures that have completed since the previous inspection.
  #[must_use]
  pub fn drain_ready_ask_futures(&self) -> Vec<ActorFutureSharedGeneric<AnyMessageGeneric<TB>, TB>> {
    self.inner.read().drain_ready_ask_futures()
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

  /// Returns the dispatcher registry.
  #[must_use]
  pub fn dispatchers(&self) -> DispatchersSharedGeneric<TB> {
    self.dispatchers.clone()
  }

  /// Returns the mailbox registry.
  #[must_use]
  pub fn mailboxes(&self) -> MailboxesSharedGeneric<TB> {
    self.mailboxes.clone()
  }

  /// Returns the remoting configuration when it has been configured.
  #[must_use]
  pub fn remoting_config(&self) -> Option<RemotingConfig> {
    self.inner.read().remoting_config()
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
  pub fn report_failure(&self, payload: FailurePayload) {
    self.inner.read().report_failure(payload);
  }

  /// Records the outcome of a previously reported failure (restart/stop/escalate).
  pub fn record_failure_outcome(&self, child: Pid, outcome: super::FailureOutcome, payload: &FailurePayload) {
    self.inner.read().record_failure_outcome(child, outcome, payload);
  }

  /// Returns a reference to the ActorPathRegistry.
  pub fn with_actor_path_registry<R, F>(&self, f: F) -> R
  where
    F: FnOnce(&super::ActorPathRegistrySharedGeneric<TB>) -> R, {
    f(&self.path_registry)
  }

  /// Returns a reference to the RemoteAuthorityManager.
  #[must_use]
  pub fn remote_authority_manager(&self) -> super::RemoteAuthorityManagerSharedGeneric<TB> {
    self.remote_mgr.clone()
  }

  /// Returns the current authority state.
  #[must_use]
  pub fn remote_authority_state(&self, authority: &str) -> AuthorityState {
    self.remote_mgr.with_read(|mgr| mgr.state(authority))
  }

  /// Returns a snapshot of known remote authorities and their states.
  #[must_use]
  pub fn remote_authority_snapshots(&self) -> Vec<(String, AuthorityState)> {
    self.remote_mgr.with_read(|mgr| mgr.snapshots())
  }

  /// Marks the authority as connected and emits an event.
  #[must_use]
  pub fn remote_authority_set_connected(&self, authority: &str) -> Option<VecDeque<AnyMessageGeneric<TB>>> {
    self.inner.read().remote_authority_set_connected(authority)
  }

  /// Transitions the authority into quarantine.
  pub fn remote_authority_set_quarantine(&self, authority: impl Into<String>, duration: Option<Duration>) {
    self.inner.read().remote_authority_set_quarantine(authority, duration);
  }

  /// Handles an InvalidAssociation signal by moving the authority into quarantine.
  pub fn remote_authority_handle_invalid_association(&self, authority: impl Into<String>, duration: Option<Duration>) {
    self.inner.read().remote_authority_handle_invalid_association(authority, duration);
  }

  /// Manually overrides a quarantined authority back to connected.
  pub fn remote_authority_manual_override_to_connected(&self, authority: &str) {
    self.inner.read().remote_authority_manual_override_to_connected(authority);
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
    self.inner.read().remote_authority_defer(authority, message)
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
    self.inner.read().remote_authority_try_defer(authority, message)
  }

  /// Polls all authorities for expired quarantine windows.
  pub fn poll_remote_authorities(&self) {
    self.inner.read().poll_remote_authorities();
  }
}

/// Type alias with the default `NoStdToolbox`.
pub type SystemStateShared = SystemStateSharedGeneric<NoStdToolbox>;
