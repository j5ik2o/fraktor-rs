//! Shared wrapper for receive-timeout runtime state.

use fraktor_utils_core_rs::core::sync::SharedLock;

use crate::core::kernel::actor::{ActorLockFactory, ReceiveTimeoutState};

/// Stable shared wrapper for receive-timeout runtime state.
#[derive(Clone)]
pub struct ReceiveTimeoutStateShared {
  inner: SharedLock<Option<ReceiveTimeoutState>>,
}

impl ReceiveTimeoutStateShared {
  /// Creates an empty receive-timeout slot with the requested lock driver family.
  #[must_use]
  pub fn new_with_lock_factory(factory: &impl ActorLockFactory) -> Self {
    Self { inner: factory.create_lock(None) }
  }

  #[must_use]
  pub(crate) const fn as_shared_lock(&self) -> &SharedLock<Option<ReceiveTimeoutState>> {
    &self.inner
  }
}
