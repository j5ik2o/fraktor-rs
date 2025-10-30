use cellactor_utils_core_rs::sync::ArcShared;

use super::dispatcher_core::DispatcherCore;

/// Public handle used to drive dispatcher execution.
#[derive(Clone)]
pub struct DispatchHandle {
  core: ArcShared<DispatcherCore>,
}

impl DispatchHandle {
  pub(super) const fn new(core: ArcShared<DispatcherCore>) -> Self {
    Self { core }
  }

  /// Runs a dispatcher batch immediately on the current thread.
  pub fn drive(&self) {
    DispatcherCore::drive(self.core.clone());
  }
}
