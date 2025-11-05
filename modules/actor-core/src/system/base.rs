//! Coordinates actors and infrastructure.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdToolbox, RuntimeToolbox,
  actor_prim::{ActorCellGeneric, ChildRefGeneric, Pid, actor_ref::ActorRefGeneric},
  dead_letter::DeadLetterEntryGeneric,
  error::SendError,
  event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric},
  futures::ActorFuture,
  logging::LogLevel,
  messaging::{AnyMessageGeneric, SystemMessage},
  props::PropsGeneric,
  spawn::SpawnError,
  system::system_state::SystemStateGeneric,
};

const PARENT_MISSING: &str = "parent actor not found";
const CREATE_SEND_FAILED: &str = "create system message delivery failed";

/// Core runtime structure that owns registry, guardians, and spawn logic.
pub struct ActorSystemGeneric<TB: RuntimeToolbox + 'static> {
  state: ArcShared<SystemStateGeneric<TB>>,
}

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
  pub fn new(user_guardian_props: &PropsGeneric<TB>) -> Result<Self, SpawnError> {
    let system = Self::new_empty();
    let guardian = system.spawn_with_parent(None, user_guardian_props)?;
    if let Some(cell) = system.state.cell(&guardian.pid()) {
      system.state.set_user_guardian(cell);
    }
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

  /// Returns the shared system state.
  #[must_use]
  pub fn state(&self) -> ArcShared<SystemStateGeneric<TB>> {
    self.state.clone()
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

  /// Emits a log event with the specified severity.
  pub fn emit_log(&self, level: LogLevel, message: impl Into<String>, origin: Option<Pid>) {
    self.state.emit_log(level, message.into(), origin);
  }

  /// Publishes a raw event to the event stream.
  pub fn publish_event(&self, event: &EventStreamEvent<TB>) {
    self.state.publish_event(event);
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

    match self.state.user_guardian_pid() {
      | Some(pid) => match self.state.send_system_message(pid, SystemMessage::Stop) {
        | Ok(()) => Ok(()),
        | Err(error) => {
          if self.state.is_terminated() {
            Ok(())
          } else {
            Err(error)
          }
        },
      },
      | None => {
        self.state.mark_terminated();
        Ok(())
      },
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
    let cell = self.build_cell_for_spawn(pid, parent, name, props);

    self.state.register_cell(cell.clone());
    self.perform_create_handshake(parent, pid, &cell)?;

    if let Some(parent_pid) = parent {
      self.state.register_child(parent_pid, pid);
    }

    Ok(ChildRefGeneric::new(cell.actor_ref(), self.state.clone()))
  }

  fn build_cell_for_spawn(
    &self,
    pid: Pid,
    parent: Option<Pid>,
    name: String,
    props: &PropsGeneric<TB>,
  ) -> ArcShared<ActorCellGeneric<TB>> {
    ActorCellGeneric::create(self.state.clone(), pid, parent, name, props)
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
}

impl<TB: RuntimeToolbox + 'static> Clone for ActorSystemGeneric<TB> {
  fn clone(&self) -> Self {
    Self { state: self.state.clone() }
  }
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for ActorSystemGeneric<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for ActorSystemGeneric<TB> {}

/// Type alias for `ActorSystemGeneric` with the default `NoStdToolbox`.
pub type ActorSystem = ActorSystemGeneric<NoStdToolbox>;
