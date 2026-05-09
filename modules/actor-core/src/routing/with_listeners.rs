//! Message that fans a callback across every registered listener.

use alloc::fmt::{Debug, Formatter, Result as FmtResult};

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::actor::actor_ref::ActorRef;

// Callback type invoked once per registered listener when a `WithListeners`
// message is handled. Ownership of the closure is shared so the envelope
// remains cheaply cloneable.
type ListenersCallback = dyn Fn(&ActorRef) + Send + Sync + 'static;

/// Carries a callback that [`Listeners::handle`](super::Listeners::handle)
/// invokes once per registered listener.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.WithListeners`.
///
/// The callback must be `Fn + Send + Sync + 'static`; coordinate mutable
/// state through an [`ArcShared`]-held container (e.g.
/// `ArcShared<SpinSyncMutex<Vec<_>>>`) to aggregate results across calls.
pub struct WithListeners {
  callback: ArcShared<ListenersCallback>,
}

impl WithListeners {
  /// Creates a new `WithListeners` that runs `callback` for each listener.
  #[must_use]
  pub fn new<F>(callback: F) -> Self
  where
    F: Fn(&ActorRef) + Send + Sync + 'static, {
    Self { callback: ArcShared::new(callback) }
  }

  /// Invokes the stored callback with the given listener reference.
  pub(crate) fn invoke(&self, actor_ref: &ActorRef) {
    (self.callback)(actor_ref);
  }
}

impl Clone for WithListeners {
  fn clone(&self) -> Self {
    Self { callback: self.callback.clone() }
  }
}

impl Debug for WithListeners {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("WithListeners").finish()
  }
}
