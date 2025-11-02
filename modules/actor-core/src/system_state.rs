//! Shared, mutable state owned by the actor system.

use alloc::{string::String, vec::Vec};
use core::time::Duration;

use cellactor_utils_core_rs::sync::{sync_mutex_like::SyncMutexLike, ArcShared, SyncMutexFamily};
use hashbrown::HashMap;
use portable_atomic::{AtomicBool, AtomicU64, Ordering};

use crate::{
  actor_cell::ActorCell, actor_error::ActorError, actor_future::ActorFuture, name_registry::NameRegistry,
  name_registry_error::NameRegistryError, pid::Pid, send_error::SendError, spawn_error::SpawnError,
  system_message::SystemMessage, AnyMessage, RuntimeToolbox, ToolboxMutex,
};

/// Captures global actor system state.
pub struct SystemState<TB: RuntimeToolbox + 'static> {
  next_pid:      AtomicU64,
  clock:         AtomicU64,
  cells:         ToolboxMutex<HashMap<Pid, ArcShared<ActorCell<TB>>>, TB>,
  registries:    ToolboxMutex<HashMap<Option<Pid>, NameRegistry>, TB>,
  user_guardian: ToolboxMutex<Option<ArcShared<ActorCell<TB>>>, TB>,
  ask_futures:   ToolboxMutex<Vec<ArcShared<ActorFuture<AnyMessage<TB>, TB>>>, TB>,
  termination:   ArcShared<ActorFuture<(), TB>>,
  terminated:    AtomicBool,
}

impl<TB: RuntimeToolbox + 'static> SystemState<TB> {
  /// Creates a fresh state container without any registered actors.
  #[must_use]
  pub fn new() -> Self {
    Self {
      next_pid:      AtomicU64::new(0),
      clock:         AtomicU64::new(0),
      cells:         <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()),
      registries:    <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()),
      user_guardian: <TB::MutexFamily as SyncMutexFamily>::create(None),
      ask_futures:   <TB::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      termination:   ArcShared::new(ActorFuture::<(), TB>::new()),
      terminated:    AtomicBool::new(false),
    }
  }

  /// Allocates a new unique [`Pid`] for an actor.
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    let value = self.next_pid.fetch_add(1, Ordering::Relaxed) + 1;
    Pid::new(value, 0)
  }

  /// Registers the provided actor cell in the global registry.
  pub fn register_cell(&self, cell: ArcShared<ActorCell<TB>>) {
    self.cells.lock().insert(cell.pid(), cell);
  }

  /// Removes the actor cell associated with the pid.
  pub fn remove_cell(&self, pid: &Pid) -> Option<ArcShared<ActorCell<TB>>> {
    self.cells.lock().remove(pid)
  }

  /// Retrieves an actor cell by pid.
  #[must_use]
  pub fn cell(&self, pid: &Pid) -> Option<ArcShared<ActorCell<TB>>> {
    self.cells.lock().get(pid).cloned()
  }

  /// Binds an actor name within its parent's scope.
  pub fn assign_name(&self, parent: Option<Pid>, hint: Option<&str>, pid: Pid) -> Result<String, SpawnError> {
    let mut registries = self.registries.lock();
    let registry = registries.entry(parent).or_insert_with(NameRegistry::new);

    match hint {
      | Some(name) => {
        registry.register(name, pid).map_err(|error| match error {
          | NameRegistryError::Duplicate(existing) => SpawnError::name_conflict(existing),
        })?;
        Ok(String::from(name))
      },
      | None => {
        let generated = registry.generate_anonymous(pid);
        registry.register(&generated, pid).map_err(|error| match error {
          | NameRegistryError::Duplicate(existing) => SpawnError::name_conflict(existing),
        })?;
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

  /// Stores the user guardian cell reference.
  pub fn set_user_guardian(&self, cell: ArcShared<ActorCell<TB>>) {
    *self.user_guardian.lock() = Some(cell);
  }

  /// Clears the guardian if the provided pid matches.
  pub fn clear_guardian(&self, pid: Pid) -> bool {
    let mut guardian = self.user_guardian.lock();
    if guardian.as_ref().map(|cell| cell.pid()) == Some(pid) {
      *guardian = None;
      return true;
    }
    false
  }

  /// Returns the user guardian cell if initialised.
  #[must_use]
  pub fn user_guardian(&self) -> Option<ArcShared<ActorCell<TB>>> {
    self.user_guardian.lock().clone()
  }

  /// Returns the pid of the user guardian if available.
  #[must_use]
  pub fn user_guardian_pid(&self) -> Option<Pid> {
    self.user_guardian.lock().as_ref().map(|cell| cell.pid())
  }

  /// Registers an ask future so the actor system can track its completion.
  pub fn register_ask_future(&self, future: ArcShared<ActorFuture<AnyMessage<TB>, TB>>) {
    self.ask_futures.lock().push(future);
  }

  /// Registers a child under the specified parent pid.
  pub fn register_child(&self, parent: Pid, child: Pid) {
    if let Some(cell) = self.cell(&parent) {
      cell.register_child(child);
    }
  }

  /// Removes a child from its parent's supervision registry.
  pub fn unregister_child(&self, parent: Option<Pid>, child: Pid) {
    if let Some(parent_pid) = parent {
      if let Some(cell) = self.cell(&parent_pid) {
        cell.unregister_child(&child);
      }
    }
  }

  /// Returns the children supervised by the specified parent pid.
  #[must_use]
  pub fn child_pids(&self, parent: Pid) -> Vec<Pid> {
    self.cell(&parent).map_or_else(Vec::new, |cell| cell.children())
  }

  /// Sends a system message to the specified actor.
  pub fn send_system_message(&self, pid: Pid, message: SystemMessage) -> Result<(), SendError<TB>> {
    if let Some(cell) = self.cell(&pid) {
      cell.dispatcher().enqueue_system(message)
    } else {
      Err(SendError::closed(AnyMessage::new(message)))
    }
  }

  /// Records a send error for diagnostics.
  pub fn record_send_error(&self, _recipient: Option<Pid>, _error: &SendError<TB>) {}

  /// Marks the system as terminated and completes the termination future.
  pub fn mark_terminated(&self) {
    if self.terminated.swap(true, Ordering::AcqRel) {
      return;
    }
    self.termination.complete(());
  }

  /// Returns a future that resolves once the actor system terminates.
  #[must_use]
  pub fn termination_future(&self) -> ArcShared<ActorFuture<(), TB>> {
    self.termination.clone()
  }

  /// Drains ask futures that have completed since the previous inspection.
  pub fn drain_ready_ask_futures(&self) -> Vec<ArcShared<ActorFuture<AnyMessage<TB>, TB>>> {
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

  /// Indicates whether the actor system has terminated.
  #[must_use]
  pub fn is_terminated(&self) -> bool {
    self.terminated.load(Ordering::Acquire)
  }

  /// Returns a monotonic timestamp for instrumentation.
  #[must_use]
  pub fn monotonic_now(&self) -> Duration {
    let ticks = self.clock.fetch_add(1, Ordering::Relaxed) + 1;
    Duration::from_millis(ticks)
  }

  /// Records a failure for future diagnostics.
  pub fn notify_failure(&self, _pid: Pid, _error: &ActorError) {}
}

impl<TB: RuntimeToolbox + 'static> Default for SystemState<TB> {
  fn default() -> Self {
    Self::new()
  }
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for SystemState<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for SystemState<TB> {}

#[cfg(test)]
mod tests {
  use alloc::string::ToString;

  use cellactor_utils_core_rs::sync::ArcShared;

  use super::SystemState;
  use crate::{actor::Actor, actor_context::ActorContext, actor_error::ActorError, AnyMessageView};

  struct ProbeActor;

  impl Actor for ProbeActor {
    fn receive(
      &mut self,
      _ctx: &mut ActorContext<'_, crate::NoStdToolbox>,
      _message: AnyMessageView<'_>,
    ) -> Result<(), ActorError> {
      Ok(())
    }
  }

  #[test]
  fn registers_and_fetches_cells() {
    let state = ArcShared::new(SystemState::<crate::NoStdToolbox>::new());
    let props = crate::props::Props::<crate::NoStdToolbox>::from_fn(|| ProbeActor);
    let pid = state.allocate_pid();
    let cell = crate::ActorCell::create(state.clone(), pid, None, "worker".to_string(), &props);
    state.register_cell(cell.clone());
    assert!(state.cell(&pid).is_some());
    state.remove_cell(&pid);
    assert!(state.cell(&pid).is_none());
  }
}
