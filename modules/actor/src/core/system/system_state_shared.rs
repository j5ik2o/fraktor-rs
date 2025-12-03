//! Shared wrapper for system state.

use alloc::{collections::VecDeque, string::String, vec::Vec};
use core::{any::TypeId, time::Duration};

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex},
  sync::ArcShared,
};

use super::{
  ActorPathRegistry, ActorRefProvider, ActorRefProviderSharedGeneric, AuthorityState, GuardianKind,
  RemoteAuthorityError, RemotingConfig, SystemStateGeneric,
};
use crate::core::{
  actor_prim::{
    ActorCellGeneric, Pid,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::ActorRefGeneric,
  },
  dead_letter::{DeadLetterEntryGeneric, DeadLetterReason},
  dispatcher::DispatchersShared,
  error::{ActorError, SendError},
  event_stream::{EventStreamEvent, EventStreamGeneric, TickDriverSnapshot},
  futures::ActorFutureSharedGeneric,
  logging::LogLevel,
  mailbox::MailboxesSharedGeneric,
  messaging::{AnyMessageGeneric, FailurePayload, SystemMessage},
  scheduler::{SchedulerContextSharedGeneric, TaskRunSummary, TickDriverRuntime},
  spawn::SpawnError,
  system::{ActorSystemConfigGeneric, RegisterExtraTopLevelError},
};

/// Shared wrapper for [`SystemStateGeneric`] providing shared ownership without external mutex.
///
/// Interior mutability is provided by individual field-level mutexes within [`SystemStateGeneric`].
pub struct SystemStateSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<SystemStateGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for SystemStateSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SystemStateSharedGeneric<TB> {
  /// Creates a new shared system state.
  #[must_use]
  pub fn new(state: SystemStateGeneric<TB>) -> Self {
    Self { inner: ArcShared::new(state) }
  }

  /// Returns the inner reference for direct access when needed.
  #[must_use]
  pub const fn inner(&self) -> &ArcShared<SystemStateGeneric<TB>> {
    &self.inner
  }

  // ====== Delegated methods from SystemStateGeneric ======

  /// Allocates a new unique [`Pid`] for an actor.
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    self.inner.allocate_pid()
  }

  /// Applies the actor system configuration.
  pub fn apply_actor_system_config(&self, config: &ActorSystemConfigGeneric<TB>) {
    self.inner.apply_actor_system_config(config);
  }

  /// Registers the provided actor cell in the global registry.
  pub fn register_cell(&self, cell: ArcShared<ActorCellGeneric<TB>>) {
    self.inner.register_cell(cell);
  }

  /// Removes the actor cell associated with the pid.
  #[must_use]
  pub fn remove_cell(&self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.inner.remove_cell(pid)
  }

  /// Returns the canonical actor path for the given pid when available.
  #[must_use]
  pub fn canonical_actor_path(&self, pid: &Pid) -> Option<ActorPath> {
    self.inner.canonical_actor_path(pid)
  }

  /// Returns the configured canonical host/port pair when remoting is enabled.
  #[must_use]
  pub fn canonical_authority_components(&self) -> Option<(String, Option<u16>)> {
    self.inner.canonical_authority_components()
  }

  /// Returns true when canonical_host is set but canonical_port is missing.
  #[must_use]
  pub fn has_partial_canonical_authority(&self) -> bool {
    self.inner.has_partial_canonical_authority()
  }

  /// Returns the canonical authority string.
  #[must_use]
  pub fn canonical_authority_endpoint(&self) -> Option<String> {
    self.inner.canonical_authority_endpoint()
  }

  /// Returns the configured actor system name.
  #[must_use]
  pub fn system_name(&self) -> String {
    self.inner.system_name()
  }

  /// Retrieves an actor cell by pid.
  #[must_use]
  pub fn cell(&self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.inner.cell(pid)
  }

  /// Binds an actor name within its parent's scope.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] if the name assignment fails.
  pub fn assign_name(&self, parent: Option<Pid>, hint: Option<&str>, pid: Pid) -> Result<String, SpawnError> {
    self.inner.assign_name(parent, hint, pid)
  }

  /// Releases the association between a name and its pid in the registry.
  pub fn release_name(&self, parent: Option<Pid>, name: &str) {
    self.inner.release_name(parent, name);
  }

  /// Stores the root guardian cell reference.
  pub fn set_root_guardian(&self, cell: ArcShared<ActorCellGeneric<TB>>) {
    self.inner.set_root_guardian(cell);
  }

  /// Stores the system guardian cell reference.
  pub fn set_system_guardian(&self, cell: ArcShared<ActorCellGeneric<TB>>) {
    self.inner.set_system_guardian(cell);
  }

  /// Stores the user guardian cell reference.
  pub fn set_user_guardian(&self, cell: ArcShared<ActorCellGeneric<TB>>) {
    self.inner.set_user_guardian(cell);
  }

  /// Clears the guardian slot matching the pid and returns which guardian stopped.
  #[must_use]
  pub fn clear_guardian(&self, pid: Pid) -> Option<GuardianKind> {
    self.inner.clear_guardian(pid)
  }

  /// Returns the root guardian cell if initialised.
  #[must_use]
  pub fn root_guardian(&self) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.inner.root_guardian()
  }

  /// Returns the system guardian cell if initialised.
  #[must_use]
  pub fn system_guardian(&self) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.inner.system_guardian()
  }

  /// Returns the user guardian cell if initialised.
  #[must_use]
  pub fn user_guardian(&self) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.inner.user_guardian()
  }

  /// Returns the pid of the root guardian if available.
  #[must_use]
  pub fn root_guardian_pid(&self) -> Option<Pid> {
    self.inner.root_guardian_pid()
  }

  /// Returns the pid of the system guardian if available.
  #[must_use]
  pub fn system_guardian_pid(&self) -> Option<Pid> {
    self.inner.system_guardian_pid()
  }

  /// Returns the pid of the user guardian if available.
  #[must_use]
  pub fn user_guardian_pid(&self) -> Option<Pid> {
    self.inner.user_guardian_pid()
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
    self.inner.register_extra_top_level(name, actor)
  }

  /// Returns a registered extra top-level reference if present.
  #[must_use]
  pub fn extra_top_level(&self, name: &str) -> Option<ActorRefGeneric<TB>> {
    self.inner.extra_top_level(name)
  }

  /// Marks the root guardian as fully initialised.
  pub fn mark_root_started(&self) {
    self.inner.mark_root_started();
  }

  /// Indicates whether the root guardian has completed startup.
  #[must_use]
  pub fn has_root_started(&self) -> bool {
    self.inner.has_root_started()
  }

  /// Attempts to transition the system into the terminating state.
  #[must_use]
  pub fn begin_termination(&self) -> bool {
    self.inner.begin_termination()
  }

  /// Indicates whether the system is currently terminating.
  #[must_use]
  pub fn is_terminating(&self) -> bool {
    self.inner.is_terminating()
  }

  /// Generates a unique `/temp` path segment and registers the supplied actor reference.
  #[must_use]
  pub fn register_temp_actor(&self, actor: ActorRefGeneric<TB>) -> String {
    self.inner.register_temp_actor(actor)
  }

  /// Removes a temporary actor reference if registered.
  #[must_use]
  pub fn unregister_temp_actor(&self, name: &str) -> Option<ActorRefGeneric<TB>> {
    self.inner.unregister_temp_actor(name)
  }

  /// Resolves a registered temporary actor reference.
  #[must_use]
  pub fn temp_actor(&self, name: &str) -> Option<ActorRefGeneric<TB>> {
    self.inner.temp_actor(name)
  }

  /// Resolves the actor path for the specified pid if the actor exists.
  #[must_use]
  pub fn actor_path(&self, pid: &Pid) -> Option<ActorPath> {
    self.inner.actor_path(pid)
  }

  /// Returns the shared event stream handle.
  #[must_use]
  pub fn event_stream(&self) -> ArcShared<EventStreamGeneric<TB>> {
    self.inner.event_stream()
  }

  /// Returns a snapshot of deadletter entries.
  #[must_use]
  pub fn dead_letters(&self) -> Vec<DeadLetterEntryGeneric<TB>> {
    self.inner.dead_letters()
  }

  /// Registers an ask future so the actor system can track its completion.
  pub fn register_ask_future(&self, future: ActorFutureSharedGeneric<AnyMessageGeneric<TB>, TB>) {
    self.inner.register_ask_future(future);
  }

  /// Publishes an event to all event stream subscribers.
  pub fn publish_event(&self, event: &EventStreamEvent<TB>) {
    self.inner.publish_event(event);
  }

  /// Emits a log event via the event stream.
  pub fn emit_log(&self, level: LogLevel, message: String, origin: Option<Pid>) {
    self.inner.emit_log(level, message, origin);
  }

  /// Returns `true` when an extension for the provided [`TypeId`] is registered.
  #[must_use]
  pub fn has_extension(&self, type_id: TypeId) -> bool {
    self.inner.has_extension(type_id)
  }

  /// Returns an extension by [`TypeId`].
  #[must_use]
  pub fn extension<E>(&self, type_id: TypeId) -> Option<ArcShared<E>>
  where
    E: core::any::Any + Send + Sync + 'static, {
    self.inner.extension(type_id)
  }

  /// Inserts an extension if absent and returns the shared instance.
  pub fn extension_or_insert_with<E, F>(&self, type_id: TypeId, factory: F) -> ArcShared<E>
  where
    E: core::any::Any + Send + Sync + 'static,
    F: FnOnce() -> ArcShared<E>, {
    self.inner.extension_or_insert_with(type_id, factory)
  }

  /// Returns an extension by its type.
  #[must_use]
  pub fn extension_by_type<E>(&self) -> Option<ArcShared<E>>
  where
    E: core::any::Any + Send + Sync + 'static, {
    self.inner.extension_by_type()
  }

  /// Installs an actor ref provider.
  pub fn install_actor_ref_provider<P>(&self, provider: &ActorRefProviderSharedGeneric<TB, P>)
  where
    P: ActorRefProvider<TB> + core::any::Any + Send + Sync + 'static, {
    self.inner.install_actor_ref_provider(provider);
  }

  /// Registers a remote watch hook.
  pub fn register_remote_watch_hook(&self, hook: alloc::boxed::Box<dyn super::RemoteWatchHook<TB>>) {
    self.inner.register_remote_watch_hook(hook);
  }

  /// Returns an actor ref provider.
  #[must_use]
  pub fn actor_ref_provider<P>(&self) -> Option<ActorRefProviderSharedGeneric<TB, P>>
  where
    P: ActorRefProvider<TB> + core::any::Any + Send + Sync + 'static, {
    self.inner.actor_ref_provider()
  }

  /// Invokes a provider registered for the given scheme.
  #[must_use]
  pub fn actor_ref_provider_call_for_scheme(
    &self,
    scheme: ActorPathScheme,
    path: ActorPath,
  ) -> Option<Result<ActorRefGeneric<TB>, ActorError>> {
    self.inner.actor_ref_provider_call_for_scheme(scheme, path)
  }

  /// Registers a child under the specified parent pid.
  pub fn register_child(&self, parent: Pid, child: Pid) {
    self.inner.register_child(parent, child);
  }

  /// Removes a child from its parent's supervision registry.
  pub fn unregister_child(&self, parent: Option<Pid>, child: Pid) {
    self.inner.unregister_child(parent, child);
  }

  /// Returns the children supervised by the specified parent pid.
  #[must_use]
  pub fn child_pids(&self, parent: Pid) -> Vec<Pid> {
    self.inner.child_pids(parent)
  }

  /// Sends a system message to the specified actor.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] if the message cannot be delivered.
  pub fn send_system_message(&self, pid: Pid, message: SystemMessage) -> Result<(), SendError<TB>> {
    self.inner.send_system_message(pid, message)
  }

  /// Records a send error for diagnostics.
  pub fn record_send_error(&self, recipient: Option<Pid>, error: &SendError<TB>) {
    self.inner.record_send_error(recipient, error);
  }

  /// Handles a failure in a child actor according to supervision strategy.
  #[allow(dead_code)]
  pub fn handle_failure(&self, pid: Pid, parent: Option<Pid>, error: &ActorError) {
    self.inner.handle_failure(pid, parent, error);
  }

  /// Records an explicit deadletter entry originating from runtime logic.
  pub fn record_dead_letter(&self, message: AnyMessageGeneric<TB>, reason: DeadLetterReason, target: Option<Pid>) {
    self.inner.record_dead_letter(message, reason, target);
  }

  /// Marks the system as terminated and completes the termination future.
  pub fn mark_terminated(&self) {
    self.inner.mark_terminated();
  }

  /// Returns a future that resolves once the actor system terminates.
  #[must_use]
  pub fn termination_future(&self) -> ActorFutureSharedGeneric<(), TB> {
    self.inner.termination_future()
  }

  /// Drains ask futures that have completed since the previous inspection.
  #[must_use]
  pub fn drain_ready_ask_futures(&self) -> Vec<ActorFutureSharedGeneric<AnyMessageGeneric<TB>, TB>> {
    self.inner.drain_ready_ask_futures()
  }

  /// Indicates whether the actor system has terminated.
  #[must_use]
  pub fn is_terminated(&self) -> bool {
    self.inner.is_terminated()
  }

  /// Returns a monotonic timestamp for instrumentation.
  #[must_use]
  pub fn monotonic_now(&self) -> Duration {
    self.inner.monotonic_now()
  }

  /// Returns the dispatcher registry.
  #[must_use]
  pub fn dispatchers(&self) -> DispatchersShared<TB> {
    self.inner.dispatchers()
  }

  /// Returns the mailbox registry.
  #[must_use]
  pub fn mailboxes(&self) -> MailboxesSharedGeneric<TB> {
    self.inner.mailboxes()
  }

  /// Returns the remoting configuration when it has been configured.
  #[must_use]
  pub fn remoting_config(&self) -> Option<RemotingConfig> {
    self.inner.remoting_config()
  }

  /// Installs the scheduler service handle.
  pub fn install_scheduler_context(&self, context: SchedulerContextSharedGeneric<TB>) {
    self.inner.install_scheduler_context(context);
  }

  /// Returns the scheduler context when it has been initialized.
  #[must_use]
  pub fn scheduler_context(&self) -> Option<SchedulerContextSharedGeneric<TB>> {
    self.inner.scheduler_context()
  }

  /// Installs the tick driver runtime.
  pub fn install_tick_driver_runtime(&self, runtime: TickDriverRuntime<TB>) {
    self.inner.install_tick_driver_runtime(runtime);
  }

  /// Returns the tick driver runtime when it has been initialized.
  #[must_use]
  pub fn tick_driver_runtime(&self) -> Option<TickDriverRuntime<TB>> {
    self.inner.tick_driver_runtime()
  }

  /// Returns the last recorded tick driver snapshot when available.
  #[must_use]
  pub fn tick_driver_snapshot(&self) -> Option<TickDriverSnapshot> {
    self.inner.tick_driver_snapshot()
  }

  /// Shuts down the scheduler context if configured.
  #[must_use]
  pub fn shutdown_scheduler(&self) -> Option<TaskRunSummary> {
    self.inner.shutdown_scheduler()
  }

  /// Records a failure and routes it to the supervising hierarchy.
  pub fn report_failure(&self, payload: FailurePayload) {
    self.inner.report_failure(payload);
  }

  /// Records the outcome of a previously reported failure (restart/stop/escalate).
  pub fn record_failure_outcome(&self, child: Pid, outcome: super::FailureOutcome, payload: &FailurePayload) {
    self.inner.record_failure_outcome(child, outcome, payload);
  }

  /// Returns a reference to the ActorPathRegistry.
  pub fn with_actor_path_registry<R, F>(&self, f: F) -> R
  where
    F: FnOnce(&ToolboxMutex<ActorPathRegistry, TB>) -> R, {
    f(self.inner.actor_path_registry())
  }

  /// Returns a reference to the RemoteAuthorityManager.
  #[must_use]
  pub fn remote_authority_manager(&self) -> super::RemoteAuthorityManagerShared<TB> {
    self.inner.remote_authority_manager().clone()
  }

  /// Returns the current authority state.
  #[must_use]
  pub fn remote_authority_state(&self, authority: &str) -> AuthorityState {
    self.inner.remote_authority_state(authority)
  }

  /// Returns a snapshot of known remote authorities and their states.
  #[must_use]
  pub fn remote_authority_snapshots(&self) -> Vec<(String, AuthorityState)> {
    self.inner.remote_authority_snapshots()
  }

  /// Marks the authority as connected and emits an event.
  #[must_use]
  pub fn remote_authority_set_connected(&self, authority: &str) -> Option<VecDeque<AnyMessageGeneric<TB>>> {
    self.inner.remote_authority_set_connected(authority)
  }

  /// Transitions the authority into quarantine.
  pub fn remote_authority_set_quarantine(&self, authority: impl Into<String>, duration: Option<Duration>) {
    self.inner.remote_authority_set_quarantine(authority, duration);
  }

  /// Handles an InvalidAssociation signal by moving the authority into quarantine.
  pub fn remote_authority_handle_invalid_association(&self, authority: impl Into<String>, duration: Option<Duration>) {
    self.inner.remote_authority_handle_invalid_association(authority, duration);
  }

  /// Manually overrides a quarantined authority back to connected.
  pub fn remote_authority_manual_override_to_connected(&self, authority: &str) {
    self.inner.remote_authority_manual_override_to_connected(authority);
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
    self.inner.remote_authority_defer(authority, message)
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
    self.inner.remote_authority_try_defer(authority, message)
  }

  /// Polls all authorities for expired quarantine windows.
  pub fn poll_remote_authorities(&self) {
    self.inner.poll_remote_authorities();
  }
}

/// Type alias with the default `NoStdToolbox`.
pub type SystemStateShared = SystemStateSharedGeneric<NoStdToolbox>;
