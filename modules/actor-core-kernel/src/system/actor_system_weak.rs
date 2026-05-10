//! Weak reference wrapper for actor system.

use super::{ActorSystem, state::SystemStateWeak};

/// Weak reference wrapper for [`ActorSystem`].
///
/// This wrapper avoids circular reference issues between actor system and components
/// that store references back to the system (such as extensions, remoting components, etc.).
pub struct ActorSystemWeak {
  pub(crate) state: SystemStateWeak,
}

impl Clone for ActorSystemWeak {
  fn clone(&self) -> Self {
    Self { state: self.state.clone() }
  }
}

impl ActorSystemWeak {
  /// Attempts to upgrade the weak reference to a strong reference.
  ///
  /// Returns `None` if the actor system has been dropped.
  #[must_use]
  pub fn upgrade(&self) -> Option<ActorSystem> {
    self.state.upgrade().map(ActorSystem::from_system_state)
  }
}
