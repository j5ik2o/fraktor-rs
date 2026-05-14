//! Registry for synthetic remote actor pids.

use std::collections::HashMap;

use fraktor_actor_core_kernel_rs::actor::{Pid, actor_path::ActorPath};
use fraktor_utils_core_rs::sync::{DefaultMutex, SharedLock};

#[derive(Default)]
pub(crate) struct RemoteActorPathRegistry {
  paths: HashMap<Pid, ActorPath>,
}

impl RemoteActorPathRegistry {
  pub(crate) fn new_shared() -> SharedLock<Self> {
    SharedLock::new_with_driver::<DefaultMutex<_>>(Self::default())
  }

  pub(crate) fn record(&mut self, pid: Pid, path: ActorPath) {
    self.paths.insert(pid, path);
  }

  pub(crate) fn path_for_pid(&self, pid: &Pid) -> Option<ActorPath> {
    self.paths.get(pid).cloned()
  }
}
