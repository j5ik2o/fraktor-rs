//! Shared wrapper for receive-timeout runtime state.

use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedLock};

use crate::core::kernel::actor::ReceiveTimeoutState;

/// Stable shared wrapper for receive-timeout runtime state.
#[derive(Clone)]
pub(crate) struct ReceiveTimeoutStateShared {
  inner: SharedLock<Option<ReceiveTimeoutState>>,
}

impl ReceiveTimeoutStateShared {
  /// Creates a new shared wrapper using the builtin spin lock backend.
  #[must_use]
  pub(crate) fn new(state: Option<ReceiveTimeoutState>) -> Self {
    Self::from_shared_lock(SharedLock::new_with_driver::<DefaultMutex<_>>(state))
  }

  /// Creates a shared wrapper from an existing shared lock.
  #[must_use]
  pub(crate) const fn from_shared_lock(inner: SharedLock<Option<ReceiveTimeoutState>>) -> Self {
    Self { inner }
  }

  #[must_use]
  pub(crate) const fn as_shared_lock(&self) -> &SharedLock<Option<ReceiveTimeoutState>> {
    &self.inner
  }
}
