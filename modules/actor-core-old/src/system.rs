use alloc::{string::String, vec::Vec};
use core::hint::spin_loop;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  actor_cell::ActorCell, actor_future::ActorFuture, actor_ref::ActorRef, any_message::AnyMessage, child_ref::ChildRef,
  deadletter_entry::DeadletterEntry, event_stream::EventStream, event_stream_subscriber::EventStreamSubscriber,
  event_stream_subscription::EventStreamSubscription, log_level::LogLevel, pid::Pid, props::Props,
  send_error::SendError, spawn_error::SpawnError, system_message::SystemMessage, system_state::ActorSystemState,
};

#[cfg(test)]
mod tests;

const ACTOR_INIT_FAILED: &str = "actor lifecycle hook failed";
const PARENT_MISSING: &str = "parent actor not found";

/// Core runtime structure that owns registry, guardians, and spawn logic.
pub struct ActorSystem {
  state: ArcShared<ActorSystemState>,
}

impl ActorSystem {
  /// Creates an empty actor system without any guardian (testing only).
  #[must_use]
  pub fn new_empty() -> Self {
    Self { state: ArcShared::new(ActorSystemState::new()) }
  }

  /// Creates a new actor system using the provided user guardian props.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when guardian initialization fails.
  pub fn new(user_guardian_props: &Props) -> Result<Self, SpawnError> {
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
  /// Panics if the user guardian has not been initialized.
  #[must_use]
  pub fn user_guardian_ref(&self) -> ActorRef {
    match self.state.user_guardian() {
      | Some(cell) => cell.actor_ref(),
      | None => panic!("user guardian has not been initialised"),
    }
  }

  /// Spawns a new top-level actor under the user guardian.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::SystemUnavailable`] when the guardian is missing.
  pub fn spawn(&self, props: &Props) -> Result<ChildRef, SpawnError> {
    let guardian_pid = self.state.user_guardian_pid().ok_or_else(SpawnError::system_unavailable)?;
    self.spawn_child(guardian_pid, props)
  }

  /// Spawns a new actor as a child of the specified parent.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::InvalidProps`] when the parent pid is unknown.
  pub fn spawn_child(&self, parent: Pid, props: &Props) -> Result<ChildRef, SpawnError> {
    if self.state.cell(&parent).is_none() {
      return Err(SpawnError::invalid_props(PARENT_MISSING));
    }
    self.spawn_with_parent(Some(parent), props)
  }

  /// Returns an [`ActorRef`] for the specified pid if the actor is registered.
  #[must_use]
  pub fn actor_ref(&self, pid: Pid) -> Option<ActorRef> {
    self.state.cell(&pid).map(|cell| cell.actor_ref())
  }

  pub(crate) const fn from_state(state: ArcShared<ActorSystemState>) -> Self {
    Self { state }
  }

  fn spawn_with_parent(&self, parent: Option<Pid>, props: &Props) -> Result<ChildRef, SpawnError> {
    let pid = self.state.allocate_pid();
    let name = self.state.assign_name(parent, props.name(), pid)?;
    let cell = ActorCell::create(self.state.clone(), pid, parent, name, props);

    self.state.register_cell(pid, cell.clone());
    if cell.pre_start().is_err() {
      self.rollback_spawn(parent, &cell, pid);
      return Err(SpawnError::invalid_props(ACTOR_INIT_FAILED));
    }

    if let Some(parent_pid) = parent {
      self.state.register_child(parent_pid, pid);
    }

    Ok(ChildRef::new(cell.actor_ref(), self.state.clone()))
  }

  /// Drains ask futures that have completed since the last call.
  #[must_use]
  pub fn drain_ready_ask_futures(&self) -> Vec<ArcShared<ActorFuture<AnyMessage>>> {
    self.state.drain_ready_ask_futures()
  }

  /// Returns the shared event stream.
  #[must_use]
  pub fn event_stream(&self) -> ArcShared<EventStream> {
    self.state.event_stream()
  }

  /// Emits a log event associated with the optional actor pid.
  pub fn emit_log(&self, level: LogLevel, message: impl Into<String>, origin: Option<Pid>) {
    self.state.emit_log(level, message.into(), origin);
  }

  /// Returns the recorded deadletter entries.
  #[must_use]
  pub fn deadletters(&self) -> Vec<DeadletterEntry> {
    self.state.deadletters()
  }

  /// Subscribes to the event stream with the provided subscriber.
  #[must_use]
  pub fn subscribe_event_stream(&self, subscriber: &ArcShared<dyn EventStreamSubscriber>) -> EventStreamSubscription {
    EventStream::subscribe_arc(&self.state.event_stream(), subscriber)
  }

  fn rollback_spawn(&self, parent: Option<Pid>, cell: &ArcShared<ActorCell>, pid: Pid) {
    self.state.release_name(parent, cell.name());
    self.state.remove_cell(&pid);
    if let Some(parent_pid) = parent {
      self.state.unregister_child(Some(parent_pid), pid);
    }
  }

  /// Returns child references supervised by the provided parent PID.
  #[must_use]
  pub fn children(&self, parent: Pid) -> Vec<ChildRef> {
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
  pub fn stop_actor(&self, pid: Pid) -> Result<(), SendError> {
    self.state.send_system_message(pid, SystemMessage::Stop)
  }

  /// Sends a stop signal to the user guardian and initiates system shutdown.
  ///
  /// # Errors
  ///
  /// Returns an error if the guardian mailbox rejects the stop request.
  pub fn terminate(&self) -> Result<(), SendError> {
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
  pub fn when_terminated(&self) -> ArcShared<ActorFuture<()>> {
    self.state.termination_future()
  }

  /// Blocks the current thread until the actor system has fully terminated.
  pub fn run_until_terminated(&self) {
    let future = self.when_terminated();
    while !future.is_ready() {
      spin_loop();
    }
  }
}

impl Clone for ActorSystem {
  fn clone(&self) -> Self {
    Self { state: self.state.clone() }
  }
}
