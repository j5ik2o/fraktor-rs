//! Registry for synthetic remote actor pids.

use std::collections::{HashMap, VecDeque};

use fraktor_actor_core_kernel_rs::actor::{Pid, actor_path::ActorPath};
use fraktor_utils_core_rs::sync::{DefaultMutex, SharedLock};

const REMOTE_PATH_REGISTRY_CAPACITY: usize = 1024;

#[derive(Default)]
pub(crate) struct RemoteActorPathRegistry {
  paths: HashMap<Pid, ActorPath>,
  pids:  HashMap<ActorPath, Pid>,
  order: VecDeque<Pid>,
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
      self.order.retain(|candidate| *candidate != previous_pid);
    }
    if !self.order.contains(&pid) {
      self.order.push_back(pid);
    }
    self.evict_if_needed();
  }

  fn evict_if_needed(&mut self) {
    while self.paths.len() > REMOTE_PATH_REGISTRY_CAPACITY {
      let Some(oldest_pid) = self.order.pop_front() else {
        break;
      };
      if let Some(oldest_path) = self.paths.remove(&oldest_pid) {
        let removed_pid = self.pids.remove(&oldest_path);
        debug_assert_eq!(removed_pid, Some(oldest_pid));
      }
    }
  }

  pub(crate) fn pid_for_path(&self, path: &ActorPath) -> Option<Pid> {
    self.pids.get(path).copied()
  }

  pub(crate) fn path_for_pid(&self, pid: &Pid) -> Option<ActorPath> {
    self.paths.get(pid).cloned()
  }
}
