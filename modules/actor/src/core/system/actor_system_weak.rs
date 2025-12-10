//! Weak reference wrapper for actor system.

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use super::{ActorSystemGeneric, SystemStateWeakGeneric};

/// Weak reference wrapper for [`ActorSystemGeneric`].
///
/// This wrapper avoids circular reference issues between actor system and components
/// that store references back to the system (such as extensions, remoting components, etc.).
pub struct ActorSystemWeakGeneric<TB: RuntimeToolbox + 'static> {
  pub(crate) state: SystemStateWeakGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> Clone for ActorSystemWeakGeneric<TB> {
  fn clone(&self) -> Self {
    Self { state: self.state.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> ActorSystemWeakGeneric<TB> {
  /// Attempts to upgrade the weak reference to a strong reference.
  ///
  /// Returns `None` if the actor system has been dropped.
  #[must_use]
  pub fn upgrade(&self) -> Option<ActorSystemGeneric<TB>> {
    self.state.upgrade().map(ActorSystemGeneric::from_state)
  }
}

/// Type alias with the default `NoStdToolbox`.
pub type ActorSystemWeak = ActorSystemWeakGeneric<NoStdToolbox>;
