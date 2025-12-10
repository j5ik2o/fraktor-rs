//! Shared wrapper for `GuardiansState`.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess},
};

use super::guardians_state::GuardiansState;

pub(crate) struct GuardiansStateSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<GuardiansState, TB>>,
}

#[allow(dead_code)]
pub(crate) type GuardiansStateShared = GuardiansStateSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> GuardiansStateSharedGeneric<TB> {
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
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut GuardiansState) -> R) -> R {
    self.inner.with_write(f)
  }
}
