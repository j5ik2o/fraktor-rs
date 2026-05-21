//! Registry for synthetic remote actor pids.

#[cfg(test)]
#[path = "remote_actor_path_registry_test.rs"]
mod tests;

use std::collections::{HashMap, VecDeque};

use fraktor_actor_core_kernel_rs::actor::{
  Pid,
  actor_path::ActorPath,
  actor_ref::{ActorRefSender, ActorRefSenderShared},
};
use fraktor_utils_core_rs::sync::{DefaultMutex, SharedLock, WeakSharedLock};

const REMOTE_PATH_REGISTRY_CAPACITY: usize = 1024;

#[derive(Default)]
pub(crate) struct RemoteActorPathRegistry {
  paths: HashMap<Pid, RemoteActorPathEntry>,
  pids:  HashMap<ActorPath, Pid>,
  order: VecDeque<Pid>,
}

impl RemoteActorPathRegistry {
  pub(crate) fn new_shared() -> SharedLock<Self> {
    SharedLock::new_with_driver::<DefaultMutex<_>>(Self::default())
  }

  pub(crate) fn record(&mut self, pid: Pid, path: ActorPath, sender: &ActorRefSenderShared) -> bool {
    if self.would_add_entry(pid, &path) && !self.reserve_new_entry_capacity() {
      return false;
    }
    if let Some(entry) = self.paths.get_mut(&pid) {
      if entry.path != path {
        let removed_pid = self.pids.remove(&entry.path);
        debug_assert_eq!(removed_pid, Some(pid));
        entry.path = path.clone();
      }
      entry.sender = sender.downgrade();
    } else {
      self.paths.insert(pid, RemoteActorPathEntry::new(path.clone(), sender.downgrade()));
      self.order.push_back(pid);
    }
    if let Some(previous_pid) = self.pids.insert(path, pid)
      && previous_pid != pid
    {
      let removed_path = self.paths.remove(&previous_pid);
      debug_assert!(removed_path.is_some());
      self.order.retain(|candidate| *candidate != previous_pid);
    }
    true
  }

  pub(crate) fn reserve_capacity_for_path(&mut self, path: &ActorPath) -> bool {
    if self.pids.contains_key(path) {
      return true;
    }
    self.reserve_new_entry_capacity()
  }

  fn would_add_entry(&self, pid: Pid, path: &ActorPath) -> bool {
    !self.paths.contains_key(&pid) && !self.pids.contains_key(path)
  }

  fn reserve_new_entry_capacity(&mut self) -> bool {
    while self.paths.len() >= REMOTE_PATH_REGISTRY_CAPACITY {
      if !self.evict_one_inactive() {
        return false;
      }
    }
    true
  }

  fn evict_one_inactive(&mut self) -> bool {
    let candidates = self.order.len();
    for _ in 0..candidates {
      let oldest_pid = self.order.pop_front().expect("remote path registry eviction candidate should exist");
      if let Some(entry) = self.paths.get(&oldest_pid) {
        if entry.is_active() {
          self.order.push_back(oldest_pid);
          continue;
        }
        let oldest_entry =
          self.paths.remove(&oldest_pid).expect("remote path registry entry should exist after lookup");
        let removed_pid = self.pids.remove(&oldest_entry.path);
        debug_assert_eq!(removed_pid, Some(oldest_pid));
        return true;
      } else {
        debug_assert!(self.paths.contains_key(&oldest_pid), "remote path registry order contained pid {oldest_pid:?}");
      }
    }
    false
  }

  pub(crate) fn pid_for_path(&self, path: &ActorPath) -> Option<Pid> {
    self.pids.get(path).copied()
  }

  pub(crate) fn path_for_pid(&self, pid: &Pid) -> Option<ActorPath> {
    self.paths.get(pid).map(|entry| entry.path.clone())
  }
}

struct RemoteActorPathEntry {
  path:   ActorPath,
  sender: WeakSharedLock<Box<dyn ActorRefSender>>,
}

impl RemoteActorPathEntry {
  const fn new(path: ActorPath, sender: WeakSharedLock<Box<dyn ActorRefSender>>) -> Self {
    Self { path, sender }
  }

  fn is_active(&self) -> bool {
    self.sender.upgrade().is_some()
  }
}
