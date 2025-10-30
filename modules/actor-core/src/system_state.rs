use alloc::string::String;

use cellactor_utils_core_rs::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};
use hashbrown::HashMap;
use portable_atomic::{AtomicU64, Ordering};

use crate::{
  actor_cell::ActorCell,
  name_registry::{NameRegistry, NameRegistryError},
  pid::Pid,
  spawn_error::SpawnError,
};

/// Shared, mutable state owned by the [`ActorSystem`](crate::system::ActorSystem).
pub struct ActorSystemState {
  next_pid:   AtomicU64,
  cells:      SpinSyncMutex<HashMap<Pid, ArcShared<ActorCell>>>,
  registries: SpinSyncMutex<HashMap<Option<Pid>, NameRegistry>>,
  guardian:   SpinSyncMutex<Option<ArcShared<ActorCell>>>,
}

impl ActorSystemState {
  /// Creates a fresh state container without any registered actors.
  #[must_use]
  pub fn new() -> Self {
    Self {
      next_pid:   AtomicU64::new(0),
      cells:      SpinSyncMutex::new(HashMap::new()),
      registries: SpinSyncMutex::new(HashMap::new()),
      guardian:   SpinSyncMutex::new(None),
    }
  }

  /// Allocates a new unique [`Pid`] for an actor.
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    let value = self.next_pid.fetch_add(1, Ordering::Relaxed) + 1;
    Pid::new(value, 0)
  }

  /// Registers the provided actor cell in the global registry.
  pub fn register_cell(&self, pid: Pid, cell: ArcShared<ActorCell>) {
    self.cells.lock().insert(pid, cell);
  }

  /// Removes the actor cell associated with the pid.
  pub fn remove_cell(&self, pid: &Pid) -> Option<ArcShared<ActorCell>> {
    self.cells.lock().remove(pid)
  }

  /// Retrieves an actor cell by pid.
  #[must_use]
  pub fn cell(&self, pid: &Pid) -> Option<ArcShared<ActorCell>> {
    self.cells.lock().get(pid).cloned()
  }

  /// Stores the user guardian cell reference.
  pub fn set_user_guardian(&self, cell: ArcShared<ActorCell>) {
    *self.guardian.lock() = Some(cell);
  }

  /// Returns the user guardian cell if initialised.
  #[must_use]
  pub fn user_guardian(&self) -> Option<ArcShared<ActorCell>> {
    self.guardian.lock().clone()
  }

  /// Reserves a name for the actor within its parent's scope.
  ///
  /// # Errors
  ///
  /// Returns `SpawnError::NameConflict` when the requested name already exists.
  pub fn assign_name(&self, parent: Option<Pid>, name_hint: Option<&str>, pid: Pid) -> Result<String, SpawnError> {
    let mut registries = self.registries.lock();
    let registry = registries.entry(parent).or_insert_with(NameRegistry::new);

    match name_hint {
      | Some(name) => {
        Self::register_name(registry, name, pid)?;
        Ok(String::from(name))
      },
      | None => {
        let generated = registry.generate_anonymous(pid);
        Self::register_name(registry, &generated, pid)?;
        Ok(generated)
      },
    }
  }

  /// Releases the association between a name and its pid in the registry.
  pub fn release_name(&self, parent: Option<Pid>, name: &str) {
    if let Some(registry) = self.registries.lock().get_mut(&parent) {
      registry.remove(name);
    }
  }

  /// Returns the pid of the user guardian if available.
  #[must_use]
  pub fn user_guardian_pid(&self) -> Option<Pid> {
    self.guardian.lock().as_ref().map(|cell| cell.pid())
  }

  fn register_name(registry: &mut NameRegistry, name: &str, pid: Pid) -> Result<(), SpawnError> {
    registry.register(name, pid).map_err(|error| match error {
      | NameRegistryError::Duplicate(existing) => SpawnError::name_conflict(existing),
    })
  }
}
