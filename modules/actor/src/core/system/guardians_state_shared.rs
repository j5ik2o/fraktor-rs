//! Shared wrapper for guardian PID slots.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::guardians_state::GuardiansState;

/// Shared wrapper for [`GuardiansState`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying guardian state, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct GuardiansStateSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<GuardiansState, TB>>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type GuardiansStateShared = GuardiansStateSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> GuardiansStateSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided guardian state.
  #[must_use]
  pub(crate) fn new(state: GuardiansState) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(state)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for GuardiansStateSharedGeneric<TB> {
  fn default() -> Self {
    Self::new(GuardiansState::default())
  }
}

impl<TB: RuntimeToolbox> Clone for GuardiansStateSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<GuardiansState> for GuardiansStateSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&GuardiansState) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut GuardiansState) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
