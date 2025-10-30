use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  actor_cell::ActorCell, actor_ref::ActorRef, pid::Pid, props::Props, spawn_error::SpawnError,
  system_state::ActorSystemState,
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
    let guardian_ref = system.spawn_with_parent(None, user_guardian_props)?;
    if let Some(cell) = system.state.cell(&guardian_ref.pid()) {
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
  pub fn spawn(&self, props: &Props) -> Result<ActorRef, SpawnError> {
    let guardian_pid = self.state.user_guardian_pid().ok_or_else(SpawnError::system_unavailable)?;
    self.spawn_child(guardian_pid, props)
  }

  /// Spawns a new actor as a child of the specified parent.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::InvalidProps`] when the parent pid is unknown.
  pub fn spawn_child(&self, parent: Pid, props: &Props) -> Result<ActorRef, SpawnError> {
    if self.state.cell(&parent).is_none() {
      return Err(SpawnError::invalid_props(PARENT_MISSING));
    }
    self.spawn_with_parent(Some(parent), props)
  }

  pub(crate) const fn from_state(state: ArcShared<ActorSystemState>) -> Self {
    Self { state }
  }

  fn spawn_with_parent(&self, parent: Option<Pid>, props: &Props) -> Result<ActorRef, SpawnError> {
    let pid = self.state.allocate_pid();
    let name = self.state.assign_name(parent, props.name(), pid)?;
    let cell = ActorCell::create(self.state.clone(), pid, parent, name, props);

    self.state.register_cell(pid, cell.clone());
    if cell.pre_start().is_err() {
      self.rollback_spawn(parent, &cell, pid);
      return Err(SpawnError::invalid_props(ACTOR_INIT_FAILED));
    }

    Ok(cell.actor_ref())
  }

  fn rollback_spawn(&self, parent: Option<Pid>, cell: &ArcShared<ActorCell>, pid: Pid) {
    self.state.release_name(parent, cell.name());
    self.state.remove_cell(&pid);
  }
}

impl Clone for ActorSystem {
  fn clone(&self) -> Self {
    Self { state: self.state.clone() }
  }
}
