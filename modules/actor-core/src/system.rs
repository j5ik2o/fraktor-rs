//! Actor system placeholder implementation.

use crate::{actor_ref::ActorRef, pid::Pid, props::Props, spawn_error::SpawnError};

/// Minimal actor system placeholder.
pub struct ActorSystem {
  _unused: (),
}

impl ActorSystem {
  /// Creates a new actor system placeholder.
  #[must_use]
  pub const fn new_empty() -> Self {
    Self { _unused: () }
  }

  /// Spawns a new actor using the supplied props.
  pub fn spawn(&self, _props: Props) {
    // 実装は後続フェーズで追加する。
  }

  /// Requests spawning a child actor for the specified parent.
  ///
  /// # Errors
  ///
  /// Returns `SpawnError::SystemUnavailable` if the actor system is not ready.
  pub fn spawn_child(&self, _parent: Pid, _props: Props) -> Result<ActorRef, SpawnError> {
    Err(SpawnError::system_unavailable())
  }
}
