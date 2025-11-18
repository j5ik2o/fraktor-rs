//! Coordinates actors and infrastructure.

#[cfg(test)]
mod tests;

use alloc::{
  string::{String, ToString},
  vec::Vec,
};
use core::any::Any;

use fraktor_utils_rs::core::{
  collections::queue::capabilities::QueueCapability,
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use super::{RootGuardianActor, SystemGuardianActor, SystemGuardianProtocol};
use crate::core::{
  actor_prim::{ActorCellGeneric, ChildRefGeneric, Pid, actor_ref::ActorRefGeneric},
  config::{ActorSystemConfig, DispatchersGeneric, MailboxesGeneric},
  dead_letter::DeadLetterEntryGeneric,
  error::SendError,
  event_stream::{
    EventStreamEvent, EventStreamGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric, TickDriverSnapshot,
  },
  extension::{Extension, ExtensionId},
  futures::ActorFuture,
  logging::LogLevel,
  messaging::{AnyMessageGeneric, SystemMessage},
  props::PropsGeneric,
  scheduler::{SchedulerBackedDelayProvider, SchedulerContext},
  spawn::SpawnError,
  system::{RegisterExtraTopLevelError, system_state::SystemStateGeneric},
};

const PARENT_MISSING: &str = "parent actor not found";
const CREATE_SEND_FAILED: &str = "create system message delivery failed";

/// Core runtime structure that owns registry, guardians, and spawn logic.
pub struct ActorSystemGeneric<TB: RuntimeToolbox + 'static> {
  state: ArcShared<SystemStateGeneric<TB>>,
}

/// Type alias for [ActorSystemGeneric] with the default [NoStdToolbox].
pub type ActorSystem = ActorSystemGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> ActorSystemGeneric<TB> {
  /// Creates an empty actor system without any guardian (testing only).
  #[must_use]
  pub fn new_empty() -> Self {
    Self { state: ArcShared::new(SystemStateGeneric::new()) }
  }

  /// Creates an actor system from an existing system state.
  #[must_use]
  pub const fn from_state(state: ArcShared<SystemStateGeneric<TB>>) -> Self {
    Self { state }
  }

  /// Creates a new actor system using the provided user guardian props.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when guardian initialization fails.
  pub fn new(user_guardian_props: &PropsGeneric<TB>) -> Result<Self, SpawnError>
  where
    TB: Default, {
    Self::new_with_config_and(user_guardian_props, &ActorSystemConfig::default(), |_| Ok(()))
  }

  /// Creates a new actor system and runs the provided configuration callback before startup.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when guardian initialization or configuration fails.
  pub fn new_with<F>(user_guardian_props: &PropsGeneric<TB>, configure: F) -> Result<Self, SpawnError>
  where
    TB: Default,
    F: FnOnce(&ActorSystemGeneric<TB>) -> Result<(), SpawnError>, {
    Self::new_with_config_and(user_guardian_props, &ActorSystemConfig::default(), configure)
  }

  /// Creates an actor system with the provided configuration.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when guardian initialization fails.
  pub fn new_with_config(
    user_guardian_props: &PropsGeneric<TB>,
    config: &ActorSystemConfig<TB>,
  ) -> Result<Self, SpawnError>
  where
    TB: Default, {
    Self::new_with_config_and(user_guardian_props, config, |_| Ok(()))
  }

  /// Creates an actor system with configuration and a bootstrap callback.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when guardian initialization or configuration fails.
  pub fn new_with_config_and<F>(
    user_guardian_props: &PropsGeneric<TB>,
    config: &ActorSystemConfig<TB>,
    configure: F,
  ) -> Result<Self, SpawnError>
  where
    TB: Default,
    F: FnOnce(&ActorSystemGeneric<TB>) -> Result<(), SpawnError>, {
    let system = Self::new_empty();
    system.state.apply_actor_system_config(config);
    system.install_scheduler_and_tick_driver_from_config(config)?;
    system.bootstrap(user_guardian_props, configure)?;
    Ok(system)
  }

  /// Returns the actor reference to the user guardian.
  ///
  /// # Panics
  ///
  /// Panics if the user guardian has not been initialised.
  #[must_use]
  pub fn user_guardian_ref(&self) -> ActorRefGeneric<TB> {
    match self.state.user_guardian() {
      | Some(cell) => cell.actor_ref(),
      | None => panic!("user guardian has not been initialised"),
    }
  }

  /// Returns the actor reference to the system guardian when available.
  #[must_use]
  pub fn system_guardian_ref(&self) -> Option<ActorRefGeneric<TB>> {
    self.state.system_guardian().map(|cell| cell.actor_ref())
  }

  /// Returns the shared system state.
  #[must_use]
  pub fn state(&self) -> ArcShared<SystemStateGeneric<TB>> {
    self.state.clone()
  }

  /// Installs scheduler context and tick driver runtime from configuration.
  ///
  /// # Errors
  ///
  /// Returns an error if tick driver provisioning fails.
  fn install_scheduler_and_tick_driver_from_config(&self, config: &ActorSystemConfig<TB>) -> Result<(), SpawnError>
  where
    TB: Default, {
    use crate::core::scheduler::TickDriverBootstrap;

    // Install scheduler context if not already present
    if self.state.scheduler_context().is_none() {
      let event_stream = self.state.event_stream();
      let toolbox = TB::default();
      let scheduler_config = *config.scheduler_config();
      let context = SchedulerContext::with_event_stream(toolbox, scheduler_config, event_stream);
      self.state.install_scheduler_context(ArcShared::new(context));
    }

    // Install tick driver runtime if tick_driver_config is provided
    if let Some(tick_driver_config) = config.tick_driver_config() {
      let ctx = self.scheduler_context().ok_or(SpawnError::SystemUnavailable)?;
      let runtime =
        TickDriverBootstrap::provision(tick_driver_config, &ctx).map_err(|_| SpawnError::SystemUnavailable)?;
      self.state.install_tick_driver_runtime(runtime);
    }

    Ok(())
  }

  /// Allocates a new pid (testing helper).
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    self.state.allocate_pid()
  }

  /// Returns the shared event stream handle.
  #[must_use]
  pub fn event_stream(&self) -> ArcShared<EventStreamGeneric<TB>> {
    self.state.event_stream()
  }

  /// Returns the scheduler service when initialized.
  #[must_use]
  pub fn scheduler_context(&self) -> Option<ArcShared<SchedulerContext<TB>>> {
    self.state.scheduler_context()
  }

  /// Returns the tick driver runtime when initialized.
  #[must_use]
  pub fn tick_driver_runtime(&self) -> Option<crate::core::scheduler::TickDriverRuntime<TB>> {
    self.state.tick_driver_runtime()
  }

  /// Returns the last reported tick driver snapshot.
  #[must_use]
  pub fn tick_driver_snapshot(&self) -> Option<TickDriverSnapshot> {
    self.state.tick_driver_snapshot()
  }

  /// Returns a delay provider backed by the scheduler when available.
  #[must_use]
  pub fn delay_provider(&self) -> Option<SchedulerBackedDelayProvider<TB>> {
    self.scheduler_context().map(|context| context.delay_provider())
  }

  /// Returns the dispatcher registry.
  #[must_use]
  pub fn dispatchers(&self) -> ArcShared<DispatchersGeneric<TB>> {
    self.state.dispatchers()
  }

  /// Returns the mailbox registry.
  #[must_use]
  pub fn mailboxes(&self) -> ArcShared<MailboxesGeneric<TB>> {
    self.state.mailboxes()
  }

  /// Subscribes the provided observer to the event stream.
  #[must_use]
  pub fn subscribe_event_stream(
    &self,
    subscriber: &ArcShared<dyn EventStreamSubscriber<TB>>,
  ) -> EventStreamSubscriptionGeneric<TB> {
    EventStreamGeneric::subscribe_arc(&self.state.event_stream(), subscriber)
  }

  /// Returns a snapshot of recorded dead letters.
  #[must_use]
  pub fn dead_letters(&self) -> Vec<DeadLetterEntryGeneric<TB>> {
    self.state.dead_letters()
  }

  /// Registers an extra top-level actor name before the system finishes startup.
  ///
  /// # Errors
  ///
  /// Returns [`RegisterExtraTopLevelError`] if the name is reserved, duplicated, or registration
  /// occurs after startup.
  pub fn register_extra_top_level(
    &self,
    name: &str,
    actor: ActorRefGeneric<TB>,
  ) -> Result<(), RegisterExtraTopLevelError> {
    self.state.register_extra_top_level(name, actor)
  }

  /// Registers a temporary actor reference under `/temp` and returns the generated segment.
  #[must_use]
  #[allow(dead_code)]
  pub(crate) fn register_temp_actor(&self, actor: ActorRefGeneric<TB>) -> String {
    self.state.register_temp_actor(actor)
  }

  /// Removes a temporary actor mapping if present.
  #[allow(dead_code)]
  pub(crate) fn unregister_temp_actor(&self, name: &str) -> Option<ActorRefGeneric<TB>> {
    self.state.unregister_temp_actor(name)
  }

  /// Resolves a registered temporary actor reference.
  #[must_use]
  #[allow(dead_code)]
  pub(crate) fn temp_actor(&self, name: &str) -> Option<ActorRefGeneric<TB>> {
    self.state.temp_actor(name)
  }

  /// Emits a log event with the specified severity.
  pub fn emit_log(&self, level: LogLevel, message: impl Into<String>, origin: Option<Pid>) {
    self.state.emit_log(level, message.into(), origin);
  }

  /// Publishes a raw event to the event stream.
  pub fn publish_event(&self, event: &EventStreamEvent<TB>) {
    self.state.publish_event(event);
  }

  /// Registers the provided extension and returns the shared instance.
  pub fn register_extension<E>(&self, ext_id: &E) -> ArcShared<E::Ext>
  where
    E: ExtensionId<TB>, {
    self.state.extension_or_insert_with(ext_id.id(), || ArcShared::new(ext_id.create_extension(self)))
  }

  /// Retrieves a previously registered extension.
  #[must_use]
  pub fn extension<E>(&self, ext_id: &E) -> Option<ArcShared<E::Ext>>
  where
    E: ExtensionId<TB>, {
    self.state.extension(ext_id.id())
  }

  /// Returns `true` when the extension has already been registered.
  #[must_use]
  pub fn has_extension<E>(&self, ext_id: &E) -> bool
  where
    E: ExtensionId<TB>, {
    self.state.has_extension(ext_id.id())
  }

  /// Registers an actor-ref provider for later retrieval.
  pub fn register_actor_ref_provider<P>(&self, provider: ArcShared<P>)
  where
    P: Any + Send + Sync + 'static, {
    self.state.install_actor_ref_provider(provider);
  }

  /// Returns the actor-ref provider of the requested type when registered.
  #[must_use]
  pub fn actor_ref_provider<P>(&self) -> Option<ArcShared<P>>
  where
    P: Any + Send + Sync + 'static, {
    self.state.actor_ref_provider::<P>()
  }

  /// Returns the extension instance by concrete type.
  #[must_use]
  pub fn extension_by_type<E>(&self) -> Option<ArcShared<E>>
  where
    E: Extension<TB> + 'static, {
    self.state.extension_by_type::<E>()
  }

  /// Spawns a new top-level actor under the user guardian.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::SystemUnavailable`] when the guardian is missing.
  #[allow(dead_code)]
  pub(crate) fn spawn(&self, props: &PropsGeneric<TB>) -> Result<ChildRefGeneric<TB>, SpawnError> {
    let guardian_pid = self.state.user_guardian_pid().ok_or_else(SpawnError::system_unavailable)?;
    self.spawn_child(guardian_pid, props)
  }

  /// Spawns a new actor as a child of the system guardian (extensions/internal subsystems).
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::SystemUnavailable`] when the system guardian is missing.
  pub fn spawn_system_actor(&self, props: &PropsGeneric<TB>) -> Result<ChildRefGeneric<TB>, SpawnError> {
    self.system_actor_of(props)
  }

  /// Spawns a new actor under the system guardian (internal use only).
  #[allow(dead_code)]
  pub(crate) fn system_actor_of(&self, props: &PropsGeneric<TB>) -> Result<ChildRefGeneric<TB>, SpawnError> {
    let guardian_pid = self.state.system_guardian_pid().ok_or_else(SpawnError::system_unavailable)?;
    self.spawn_child(guardian_pid, props)
  }

  /// Spawns a new actor as a child of the specified parent.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::InvalidProps`] when the parent pid is unknown.
  pub(crate) fn spawn_child(&self, parent: Pid, props: &PropsGeneric<TB>) -> Result<ChildRefGeneric<TB>, SpawnError> {
    if self.state.cell(&parent).is_none() {
      return Err(SpawnError::invalid_props(PARENT_MISSING));
    }
    self.spawn_with_parent(Some(parent), props)
  }

  /// Returns an [`ActorRef`] for the specified pid if the actor is registered.
  #[must_use]
  pub(crate) fn actor_ref(&self, pid: Pid) -> Option<ActorRefGeneric<TB>> {
    self.state.cell(&pid).map(|cell| cell.actor_ref())
  }

  /// Returns child references supervised by the provided parent PID.
  #[must_use]
  pub(crate) fn children(&self, parent: Pid) -> Vec<ChildRefGeneric<TB>> {
    let system = self.state.clone();
    self
      .state
      .child_pids(parent)
      .into_iter()
      .filter_map(|pid| self.state.cell(&pid).map(|cell| ChildRefGeneric::new(cell.actor_ref(), system.clone())))
      .collect()
  }

  /// Sends a stop signal to the specified actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the stop message cannot be enqueued.
  pub(crate) fn stop_actor(&self, pid: Pid) -> Result<(), SendError<TB>> {
    self.state.send_system_message(pid, SystemMessage::Stop)
  }

  /// Drains ask futures that have been fulfilled since the last check.
  #[must_use]
  pub fn drain_ready_ask_futures(&self) -> Vec<ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>> {
    self.state.drain_ready_ask_futures()
  }

  /// Sends a stop signal to the user guardian and initiates system shutdown.
  ///
  /// # Errors
  ///
  /// Returns an error if the guardian mailbox rejects the stop request.
  pub fn terminate(&self) -> Result<(), SendError<TB>> {
    if self.state.is_terminated() {
      return Ok(());
    }

    if self.state.begin_termination() {
      let _ = self.state.shutdown_scheduler();
      if let Some(root_pid) = self.state.root_guardian_pid() {
        if let Some(user_pid) = self.state.user_guardian_pid() {
          return self.state.send_system_message(root_pid, SystemMessage::StopChild(user_pid));
        }
        self.state.mark_terminated();
        return Ok(());
      }
      if let Some(user_pid) = self.state.user_guardian_pid() {
        return self.state.send_system_message(user_pid, SystemMessage::Stop);
      }
      self.state.mark_terminated();
      Ok(())
    } else {
      self.force_termination_hooks();
      Ok(())
    }
  }

  /// Returns a future that resolves once the actor system terminates.
  #[must_use]
  pub fn when_terminated(&self) -> ArcShared<ActorFuture<(), TB>> {
    self.state.termination_future()
  }

  /// Blocks the current thread until the actor system has fully terminated.
  pub fn run_until_terminated(&self) {
    let future = self.when_terminated();
    while !future.is_ready() {
      core::hint::spin_loop();
    }
  }

  fn spawn_with_parent(
    &self,
    parent: Option<Pid>,
    props: &PropsGeneric<TB>,
  ) -> Result<ChildRefGeneric<TB>, SpawnError> {
    let pid = self.state.allocate_pid();
    let name = self.state.assign_name(parent, props.name(), pid)?;
    let cell = self.build_cell_for_spawn(pid, parent, name, props)?;

    self.state.register_cell(cell.clone());
    self.perform_create_handshake(parent, pid, &cell)?;

    if let Some(parent_pid) = parent {
      self.state.register_child(parent_pid, pid);
    }

    Ok(ChildRefGeneric::new(cell.actor_ref(), self.state.clone()))
  }

  fn bootstrap<F>(&self, user_guardian_props: &PropsGeneric<TB>, configure: F) -> Result<(), SpawnError>
  where
    F: FnOnce(&ActorSystemGeneric<TB>) -> Result<(), SpawnError>, {
    let root_props = PropsGeneric::from_fn(RootGuardianActor::new).with_name("root");
    let root_cell = self.spawn_root_guardian_cell(&root_props)?;
    let root_pid = root_cell.pid();
    self.state.set_root_guardian(root_cell.clone());

    let user_guardian = self.spawn_child(root_pid, user_guardian_props)?;
    if let Some(cell) = self.state.cell(&user_guardian.pid()) {
      self.state.set_user_guardian(cell);
    } else {
      return Err(SpawnError::invalid_props("user guardian unavailable"));
    }

    let user_guardian_ref = user_guardian.actor_ref();
    let system_props = PropsGeneric::from_fn({
      let user_guardian_ref = user_guardian_ref.clone();
      move || SystemGuardianActor::new(user_guardian_ref.clone())
    })
    .with_name("system");

    let system_guardian = self.spawn_child(root_pid, &system_props)?;
    if let Some(cell) = self.state.cell(&system_guardian.pid()) {
      self.state.set_system_guardian(cell);
    }

    // TODO: enable serialization extension
    // let _ = self.register_extension(&SERIALIZATION_EXTENSION);

    configure(self)?;

    if let Err(error) = self.perform_create_handshake(None, root_pid, &root_cell) {
      self.rollback_spawn(None, &root_cell, root_pid);
      return Err(error);
    }
    self.state.mark_root_started();
    Ok(())
  }

  fn spawn_root_guardian_cell(&self, props: &PropsGeneric<TB>) -> Result<ArcShared<ActorCellGeneric<TB>>, SpawnError> {
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
    props: &PropsGeneric<TB>,
  ) -> Result<ArcShared<ActorCellGeneric<TB>>, SpawnError> {
    let resolved = self.resolve_props(props)?;
    Self::ensure_mailbox_requirements(&resolved)?;
    ActorCellGeneric::create(self.state.clone(), pid, parent, name, &resolved)
  }

  fn ensure_mailbox_requirements(props: &PropsGeneric<TB>) -> Result<(), SpawnError> {
    let requirement = props.mailbox().requirement();
    let registry = props.mailbox().capabilities();
    requirement.ensure_supported(&registry).map_err(|error| {
      let reason = Self::missing_capability_reason(error.missing());
      SpawnError::invalid_props(reason)
    })
  }

  const fn missing_capability_reason(capability: QueueCapability) -> &'static str {
    match capability {
      | QueueCapability::Mpsc => "mailbox requires MPSC capability",
      | QueueCapability::Deque => "mailbox requires deque capability",
      | QueueCapability::BlockingFuture => "mailbox requires blocking-future capability",
    }
  }

  fn resolve_props(&self, props: &PropsGeneric<TB>) -> Result<PropsGeneric<TB>, SpawnError> {
    let mut resolved = props.clone();
    if let Some(dispatcher_id) = resolved.dispatcher_id() {
      let config = self
        .state
        .dispatchers()
        .resolve(dispatcher_id)
        .map_err(|error| SpawnError::invalid_props(error.to_string()))?;
      resolved = resolved.with_resolved_dispatcher(config);
    }
    if let Some(mailbox_id) = resolved.mailbox_id() {
      let config =
        self.state.mailboxes().resolve(mailbox_id).map_err(|error| SpawnError::invalid_props(error.to_string()))?;
      resolved = resolved.with_resolved_mailbox(config);
    }
    Ok(resolved)
  }

  fn perform_create_handshake(
    &self,
    parent: Option<Pid>,
    pid: Pid,
    cell: &ArcShared<ActorCellGeneric<TB>>,
  ) -> Result<(), SpawnError> {
    if let Err(error) = self.state.send_system_message(pid, SystemMessage::Create) {
      self.state.record_send_error(Some(pid), &error);
      self.rollback_spawn(parent, cell, pid);
      return Err(SpawnError::invalid_props(CREATE_SEND_FAILED));
    }

    Ok(())
  }

  fn rollback_spawn(&self, parent: Option<Pid>, cell: &ArcShared<ActorCellGeneric<TB>>, pid: Pid) {
    self.state.release_name(parent, cell.name());
    self.state.remove_cell(&pid);
    if let Some(parent_pid) = parent {
      self.state.unregister_child(Some(parent_pid), pid);
    }
  }

  fn force_termination_hooks(&self) {
    if let Some(system_pid) = self.state.system_guardian_pid()
      && let Some(system_ref) = self.actor_ref(system_pid)
    {
      let _ = system_ref.tell(AnyMessageGeneric::new(SystemGuardianProtocol::<TB>::ForceTerminateHooks));
    }
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for ActorSystemGeneric<TB> {
  fn clone(&self) -> Self {
    Self { state: self.state.clone() }
  }
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for ActorSystemGeneric<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for ActorSystemGeneric<TB> {}
