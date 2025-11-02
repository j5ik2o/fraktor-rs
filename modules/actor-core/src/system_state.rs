//! Shared, mutable state owned by the actor system.

use alloc::string::String;
use cellactor_utils_core_rs::sync::{sync_mutex_like::SyncMutexLike, ArcShared, SyncMutexFamily};
use hashbrown::HashMap;
use portable_atomic::{AtomicU64, Ordering};

use crate::{actor_cell::ActorCell, pid::Pid, RuntimeToolbox, ToolboxMutex};

/// Captures global actor system state.
pub struct SystemState<TB: RuntimeToolbox + 'static> {
  next_pid: AtomicU64,
  cells:    ToolboxMutex<HashMap<Pid, ArcShared<ActorCell<TB>>>, TB>,
  names:    ToolboxMutex<HashMap<Option<Pid>, HashMap<String, Pid>>, TB>,
}

impl<TB: RuntimeToolbox + 'static> SystemState<TB> {
  /// Creates a fresh state container without any registered actors.
  #[must_use]
  pub fn new() -> Self {
    Self {
      next_pid: AtomicU64::new(0),
      cells: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()),
      names: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()),
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
  pub fn assign_name(&self, parent: Option<Pid>, name: String, pid: Pid) {
    let mut registries = self.names.lock();
    registries.entry(parent).or_default().insert(name, pid);
  }

  /// Releases the association between a name and its pid in the registry.
  pub fn release_name(&self, parent: Option<Pid>, name: &str) {
    if let Some(registry) = self.names.lock().get_mut(&parent) {
      registry.remove(name);
    }
  }
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

  use crate::{
    actor_cell::ActorCell,
    dispatcher::Dispatcher,
    mailbox::{Mailbox, MailboxPolicy},
    pid::Pid,
    RuntimeToolbox,
  };

  use super::SystemState;

  fn build_cell<TB: RuntimeToolbox + 'static>(state: &SystemState<TB>) -> ArcShared<ActorCell<TB>> {
    let mailbox = ArcShared::new(Mailbox::new(MailboxPolicy::unbounded(None)));
    let dispatcher = Dispatcher::with_inline_executor(mailbox.clone());
    ArcShared::new(ActorCell::new(state.allocate_pid(), None, "worker".to_string(), mailbox, dispatcher))
  }

  #[test]
  fn registers_and_fetches_cells() {
    let state: SystemState<crate::NoStdToolbox> = SystemState::new();
    let cell = build_cell(&state);
    let pid = cell.pid();
    state.register_cell(cell.clone());
    assert!(state.cell(&pid).is_some());
    state.remove_cell(&pid);
    assert!(state.cell(&pid).is_none());
  }
}
