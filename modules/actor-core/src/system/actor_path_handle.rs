//! Actor path handle for registry caching.

use crate::actor_prim::{Pid, actor_path::ActorUid};

/// Handle for cached actor path with UID-independent hash.
#[derive(Clone, Debug)]
pub struct ActorPathHandle {
  pid:           Pid,
  canonical_uri: alloc::string::String,
  uid:           Option<ActorUid>,
}

impl ActorPathHandle {
  /// Creates a new handle with given PID, canonical URI and optional UID.
  pub(crate) const fn new(pid: Pid, canonical_uri: alloc::string::String, uid: Option<ActorUid>) -> Self {
    Self { pid, canonical_uri, uid }
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
}
