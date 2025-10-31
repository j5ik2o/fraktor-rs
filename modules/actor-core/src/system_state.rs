use alloc::{string::String, vec::Vec};
use core::time::Duration;

use cellactor_utils_core_rs::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};
use hashbrown::HashMap;
use portable_atomic::{AtomicBool, AtomicU64, Ordering};

use crate::{
  actor_cell::ActorCell,
  actor_error::ActorError,
  actor_future::ActorFuture,
  any_message::AnyMessage,
  name_registry::{NameRegistry, NameRegistryError},
  pid::Pid,
  send_error::SendError,
  spawn_error::SpawnError,
  supervisor_strategy::SupervisorDirective,
  system_message::SystemMessage,
};

/// Shared, mutable state owned by the [`ActorSystem`](crate::system::ActorSystem).
pub struct ActorSystemState {
  next_pid:    AtomicU64,
  clock:       AtomicU64,
  cells:       SpinSyncMutex<HashMap<Pid, ArcShared<ActorCell>>>,
  registries:  SpinSyncMutex<HashMap<Option<Pid>, NameRegistry>>,
  guardian:    SpinSyncMutex<Option<ArcShared<ActorCell>>>,
  ask_futures: SpinSyncMutex<Vec<ArcShared<ActorFuture<AnyMessage>>>>,
  termination: ArcShared<ActorFuture<()>>,
  terminated:  AtomicBool,
}

impl ActorSystemState {
  /// Creates a fresh state container without any registered actors.
  #[must_use]
  pub fn new() -> Self {
    Self {
      next_pid:    AtomicU64::new(0),
      clock:       AtomicU64::new(0),
      cells:       SpinSyncMutex::new(HashMap::new()),
      registries:  SpinSyncMutex::new(HashMap::new()),
      guardian:    SpinSyncMutex::new(None),
      ask_futures: SpinSyncMutex::new(Vec::new()),
      termination: ArcShared::new(ActorFuture::new()),
      terminated:  AtomicBool::new(false),
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

  /// Clears the guardian if the provided pid matches.
  pub fn clear_guardian(&self, pid: Pid) -> bool {
    let mut guard = self.guardian.lock();
    if guard.as_ref().map(|cell| cell.pid()) == Some(pid) {
      *guard = None;
      true
    } else {
      false
    }
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
    let registry = registries.entry(parent).or_default();

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

  /// Registers an ask future so the actor system can track its completion.
  pub fn register_ask_future(&self, future: ArcShared<ActorFuture<AnyMessage>>) {
    self.ask_futures.lock().push(future);
  }

  /// Returns the termination future for when the actor system shuts down.
  #[must_use]
  pub fn termination_future(&self) -> ArcShared<ActorFuture<()>> {
    self.termination.clone()
  }

  /// Marks the system as terminated and completes the termination future.
  pub fn mark_terminated(&self) {
    if self.terminated.swap(true, Ordering::AcqRel) {
      return;
    }
    self.termination.complete(());
  }

  /// Drains futures that have completed since the previous inspection.
  #[must_use]
  pub fn drain_ready_ask_futures(&self) -> Vec<ArcShared<ActorFuture<AnyMessage>>> {
    let mut registry = self.ask_futures.lock();
    let mut ready = Vec::new();
    let mut index = 0_usize;

    while index < registry.len() {
      if registry[index].is_ready() {
        ready.push(registry.swap_remove(index));
      } else {
        index += 1;
      }
    }

    ready
  }

  fn register_name(registry: &mut NameRegistry, name: &str, pid: Pid) -> Result<(), SpawnError> {
    registry.register(name, pid).map_err(|error| match error {
      | NameRegistryError::Duplicate(existing) => SpawnError::name_conflict(existing),
    })
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
  pub fn send_system_message(&self, pid: Pid, message: SystemMessage) -> Result<(), SendError> {
    if let Some(cell) = self.cell(&pid) {
      cell.dispatcher().enqueue_system(message)
    } else {
      Err(SendError::closed(AnyMessage::new(message)))
    }
  }

  /// Handles an actor failure by applying the appropriate supervisor directive.
  pub fn notify_failure(&self, pid: Pid, error: &ActorError) {
    let parent = self.parent_of(&pid);
    self.handle_failure(pid, parent, error);
  }

  fn handle_failure(&self, pid: Pid, parent: Option<Pid>, error: &ActorError) {
    let Some(parent_pid) = parent else {
      self.stop_actor(pid);
      return;
    };

    let Some(parent_cell) = self.cell(&parent_pid) else {
      self.stop_actor(pid);
      return;
    };

    let parent_parent = parent_cell.parent();
    let now = self.monotonic_now();
    let (directive, affected) = parent_cell.handle_child_failure(pid, error, now);

    match directive {
      | SupervisorDirective::Restart => {
        for target in affected {
          let _ = self.restart_actor(target);
        }
      },
      | SupervisorDirective::Stop => {
        for target in affected {
          self.stop_actor(target);
        }
      },
      | SupervisorDirective::Escalate => {
        for target in affected {
          self.stop_actor(target);
        }
        self.handle_failure(parent_pid, parent_parent, error);
      },
    }
  }

  fn restart_actor(&self, pid: Pid) -> Result<(), ActorError> {
    if let Some(cell) = self.cell(&pid) { cell.restart() } else { Ok(()) }
  }

  fn stop_actor(&self, pid: Pid) {
    if let Some(cell) = self.cell(&pid) {
      let _ = cell.dispatcher().enqueue_system(SystemMessage::Stop);
    }
  }

  fn parent_of(&self, pid: &Pid) -> Option<Pid> {
    self.cell(pid).and_then(|cell| cell.parent())
  }

  fn monotonic_now(&self) -> Duration {
    let ticks = self.clock.fetch_add(1, Ordering::Relaxed) + 1;
    Duration::from_millis(ticks)
  }

  /// Indicates whether the actor system has terminated.
  #[must_use]
  pub fn is_terminated(&self) -> bool {
    self.terminated.load(Ordering::Acquire)
  }
}
