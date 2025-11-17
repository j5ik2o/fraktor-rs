//! Actor path handle for registry caching.

use crate::core::actor_prim::{Pid, actor_path::ActorUid};

/// Handle for cached actor path with UID-independent hash.
#[derive(Clone, Debug)]
pub struct ActorPathHandle {
  pid:           Pid,
  canonical_uri: alloc::string::String,
  uid:           Option<ActorUid>,
  path_hash:     u64,
}

impl ActorPathHandle {
  /// Creates a new handle with given PID, canonical URI and optional UID.
  pub(crate) const fn new(
    pid: Pid,
    canonical_uri: alloc::string::String,
    uid: Option<ActorUid>,
    path_hash: u64,
  ) -> Self {
    Self { pid, canonical_uri, uid, path_hash }
  }

  /// Returns the PID associated with this handle.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the canonical URI.
  #[must_use]
  pub fn canonical_uri(&self) -> &str {
    &self.canonical_uri
  }

  /// Returns the UID if present.
  #[must_use]
  pub const fn uid(&self) -> Option<ActorUid> {
    self.uid
  }

  /// Returns the UID-independent hash for the canonical path.
  #[must_use]
  pub const fn path_hash(&self) -> u64 {
    self.path_hash
  }
}
