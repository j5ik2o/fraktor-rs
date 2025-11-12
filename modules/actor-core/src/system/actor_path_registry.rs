//! Registry for mapping PIDs to canonical actor paths.

use hashbrown::HashMap;

use crate::actor_prim::{
  actor_path::{ActorPath, ActorUid},
  Pid,
};

/// Handle for cached actor path with UID-independent hash.
#[derive(Clone, Debug)]
pub struct ActorPathHandle {
  pid:           Pid,
  canonical_uri: alloc::string::String,
  uid:           Option<ActorUid>,
}

impl ActorPathHandle {
  /// Returns the PID associated with this handle.
  #[must_use]
  pub fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the canonical URI.
  #[must_use]
  pub fn canonical_uri(&self) -> &str {
    &self.canonical_uri
  }

  /// Returns the UID if present.
  #[must_use]
  pub fn uid(&self) -> Option<ActorUid> {
    self.uid
  }
}

/// Registry for PID-to-path mappings and UID reservations.
pub struct ActorPathRegistry {
  paths: HashMap<Pid, ActorPathHandle>,
}

impl ActorPathRegistry {
  /// Creates a new empty registry.
  #[must_use]
  pub fn new() -> Self {
    Self {
      paths: HashMap::new(),
    }
  }

  /// Registers a path for a given PID.
  pub fn register(&mut self, pid: Pid, path: &ActorPath) {
    let handle = ActorPathHandle {
      pid,
      canonical_uri: path.to_canonical_uri(),
      uid: path.uid(),
    };
    self.paths.insert(pid, handle);
  }

  /// Retrieves a path handle by PID.
  #[must_use]
  pub fn get(&self, pid: &Pid) -> Option<&ActorPathHandle> {
    self.paths.get(pid)
  }

  /// Removes a path registration.
  pub fn unregister(&mut self, pid: &Pid) {
    self.paths.remove(pid);
  }

  /// Returns the canonical URI for a PID.
  #[must_use]
  pub fn canonical_uri(&self, pid: &Pid) -> Option<&str> {
    self.get(pid).map(ActorPathHandle::canonical_uri)
  }
}

impl Default for ActorPathRegistry {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests;
