use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

/// Control handle for initiating hub draining.
#[derive(Clone)]
pub struct DrainingControl {
  draining: ArcShared<SpinSyncMutex<bool>>,
}

impl DrainingControl {
  pub(in crate::core::hub) const fn new(draining: ArcShared<SpinSyncMutex<bool>>) -> Self {
    Self { draining }
  }

  /// Starts draining mode.
  pub fn drain(&self) {
    *self.draining.lock() = true;
  }

  /// Returns true when draining mode is active.
  #[must_use]
  pub fn is_draining(&self) -> bool {
    *self.draining.lock()
  }
}
