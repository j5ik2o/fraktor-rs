//! Shared wrapper for `GuardiansState`.

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess},
};

use super::guardians_state::GuardiansState;

pub(crate) struct GuardiansStateShared<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<GuardiansState, TB>>,
}

pub(crate) type GuardiansStateSharedGeneric<TB> = GuardiansStateShared<TB>;
impl<TB: RuntimeToolbox + 'static> GuardiansStateShared<TB> {
  pub(crate) fn new(state: GuardiansState) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(state)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for GuardiansStateShared<TB> {
  fn default() -> Self {
    Self::new(GuardiansState::default())
  }
}

impl<TB: RuntimeToolbox> Clone for GuardiansStateShared<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<GuardiansState> for GuardiansStateShared<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&GuardiansState) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut GuardiansState) -> R) -> R {
    self.inner.with_write(f)
  }
}
