//! Coordinates actors and infrastructure.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdToolbox, RuntimeToolbox,
  actor_prim::{ActorCell, ChildRef, Pid, actor_ref::ActorRef},
  deadletter::DeadletterEntry,
  error::SendError,
  eventstream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric},
  futures::ActorFuture,
  logging::LogLevel,
  messaging::{AnyMessage, SystemMessage},
  props::Props,
  spawn::SpawnError,
  system::system_state::SystemState,
};

const ACTOR_INIT_FAILED: &str = "actor lifecycle hook failed";
const PARENT_MISSING: &str = "parent actor not found";

/// Core runtime structure that owns registry, guardians, and spawn logic.
pub struct ActorSystemGeneric<TB: RuntimeToolbox + 'static = NoStdToolbox> {
  state: ArcShared<SystemState<TB>>,
}

impl<TB: RuntimeToolbox + 'static> ActorSystemGeneric<TB> {
  /// Creates an empty actor system without any guardian (testing only).
  #[must_use]
  pub fn new_empty() -> Self {
    Self { state: ArcShared::new(SystemState::new()) }
  }

  /// Creates an actor system from an existing system state.
  #[must_use]
  pub const fn from_state(state: ArcShared<SystemState<TB>>) -> Self {
    Self { state }
  }

  /// Creates a new actor system using the provided user guardian props.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when guardian initialization fails.
  pub fn new(user_guardian_props: &Props<TB>) -> Result<Self, SpawnError> {
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
  pub fn user_guardian_ref(&self) -> ActorRef<TB> {
    match self.state.user_guardian() {
      | Some(cell) => cell.actor_ref(),
      | None => panic!("user guardian has not been initialised"),
    }
  }

  /// Returns the shared system state.
  #[must_use]
  pub fn state(&self) -> ArcShared<SystemState<TB>> {
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

  /// Returns a snapshot of recorded deadletters.
  #[must_use]
  pub fn deadletters(&self) -> Vec<DeadletterEntry<TB>> {
    self.state.deadletters()
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
  pub fn spawn(&self, props: &Props<TB>) -> Result<ChildRef<TB>, SpawnError> {
    let guardian_pid = self.state.user_guardian_pid().ok_or_else(SpawnError::system_unavailable)?;
    self.spawn_child(guardian_pid, props)
  }

  /// Spawns a new actor as a child of the specified parent.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::InvalidProps`] when the parent pid is unknown.
  pub fn spawn_child(&self, parent: Pid, props: &Props<TB>) -> Result<ChildRef<TB>, SpawnError> {
    if self.state.cell(&parent).is_none() {
      return Err(SpawnError::invalid_props(PARENT_MISSING));
    }
    self.spawn_with_parent(Some(parent), props)
  }

  /// Returns an [`ActorRef`] for the specified pid if the actor is registered.
  #[must_use]
  pub fn actor_ref(&self, pid: Pid) -> Option<ActorRef<TB>> {
    self.state.cell(&pid).map(|cell| cell.actor_ref())
  }

  /// Returns child references supervised by the provided parent PID.
  #[must_use]
  pub fn children(&self, parent: Pid) -> Vec<ChildRef<TB>> {
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
  pub fn stop_actor(&self, pid: Pid) -> Result<(), SendError<TB>> {
    self.state.send_system_message(pid, SystemMessage::Stop)
  }

  /// Drains ask futures that have been fulfilled since the last check.
  #[must_use]
  pub fn drain_ready_ask_futures(&self) -> Vec<ArcShared<ActorFuture<AnyMessage<TB>, TB>>> {
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

  fn spawn_with_parent(&self, parent: Option<Pid>, props: &Props<TB>) -> Result<ChildRef<TB>, SpawnError> {
    let pid = self.state.allocate_pid();
    let name = self.state.assign_name(parent, props.name(), pid)?;
    let cell = ActorCell::create(self.state.clone(), pid, parent, name, props);

    self.state.register_cell(cell.clone());
    if cell.pre_start().is_err() {
      self.rollback_spawn(parent, &cell, pid);
      return Err(SpawnError::invalid_props(ACTOR_INIT_FAILED));
    }

    if let Some(parent_pid) = parent {
      self.state.register_child(parent_pid, pid);
    }

    Ok(ChildRef::new(cell.actor_ref(), self.state.clone()))
  }

  fn rollback_spawn(&self, parent: Option<Pid>, cell: &ArcShared<ActorCell<TB>>, pid: Pid) {
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

/// Type alias for compatibility with older code.
pub type ActorSystem<TB> = ActorSystemGeneric<TB>;
