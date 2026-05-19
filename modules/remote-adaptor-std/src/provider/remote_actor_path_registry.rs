//! Registry for synthetic remote actor pids.

use std::collections::HashMap;

use fraktor_actor_core_kernel_rs::actor::{Pid, actor_path::ActorPath};
use fraktor_utils_core_rs::sync::{DefaultMutex, SharedLock};

#[derive(Default)]
pub(crate) struct RemoteActorPathRegistry {
  paths: HashMap<Pid, ActorPath>,
  pids:  HashMap<ActorPath, Pid>,
}

impl RemoteActorPathRegistry {
  pub(crate) fn new_shared() -> SharedLock<Self> {
    SharedLock::new_with_driver::<DefaultMutex<_>>(Self::default())
  }

  pub(crate) fn record(&mut self, pid: Pid, path: ActorPath) {
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
  }

  pub(crate) fn pid_for_path(&self, path: &ActorPath) -> Option<Pid> {
    self.pids.get(path).copied()
  }

  pub(crate) fn path_for_pid(&self, pid: &Pid) -> Option<ActorPath> {
    self.paths.get(pid).cloned()
  }
}
