//! Registry for synthetic remote actor pids.

use std::collections::HashMap;

use fraktor_actor_core_kernel_rs::actor::{Pid, actor_path::ActorPath};
use fraktor_utils_core_rs::sync::{DefaultMutex, SharedLock};

const REMOTE_PATH_REGISTRY_CAPACITY: usize = 1024;

#[derive(Default)]
pub(crate) struct RemoteActorPathRegistry {
  paths: HashMap<Pid, ActorPath>,
  pids:  HashMap<ActorPath, Pid>,
}

impl RemoteActorPathRegistry {
  pub(crate) fn new_shared() -> SharedLock<Self> {
    SharedLock::new_with_driver::<DefaultMutex<_>>(Self::default())
  }

  pub(crate) fn record(&mut self, pid: Pid, path: ActorPath) -> bool {
    if self.would_add_entry(pid, &path) && !self.has_new_entry_capacity() {
      return false;
    }
    if let Some(previous_path) = self.paths.insert(pid, path.clone())
      && previous_path != path
    {
      let removed_pid = self.pids.remove(&previous_path);
      debug_assert_eq!(removed_pid, Some(pid));
    }
    if let Some(previous_pid) = self.pids.insert(path, pid)
      && previous_pid != pid
    {
      let removed_path = self.paths.remove(&previous_pid);
      debug_assert!(removed_path.is_some());
    }
    true
  }

  pub(crate) fn refresh(&mut self, pid: Pid, path: ActorPath) -> bool {
    self.record(pid, path)
  }

  pub(crate) fn can_record_path(&self, path: &ActorPath) -> bool {
    if self.pids.contains_key(path) {
      return true;
    }
    self.has_new_entry_capacity()
  }

  fn would_add_entry(&self, pid: Pid, path: &ActorPath) -> bool {
    !self.paths.contains_key(&pid) && !self.pids.contains_key(path)
  }

  fn has_new_entry_capacity(&self) -> bool {
    self.paths.len() < REMOTE_PATH_REGISTRY_CAPACITY
  }

  pub(crate) fn pid_for_path(&self, path: &ActorPath) -> Option<Pid> {
    self.pids.get(path).copied()
  }

  pub(crate) fn path_for_pid(&self, pid: &Pid) -> Option<ActorPath> {
    self.paths.get(pid).cloned()
  }
}
