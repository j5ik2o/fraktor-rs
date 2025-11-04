use cellactor_utils_core_rs::sync::ArcShared;

use super::dispatcher_core::DispatcherCore;
use crate::RuntimeToolbox;

/// Public handle used to drive dispatcher execution.
#[derive(Clone)]
pub struct DispatchHandle<TB: RuntimeToolbox + 'static> {
  core: ArcShared<DispatcherCore<TB>>,
}

impl<TB: RuntimeToolbox + 'static> DispatchHandle<TB> {
  pub(super) const fn new(core: ArcShared<DispatcherCore<TB>>) -> Self {
    Self { core }
  }

  /// Runs a dispatcher batch immediately on the current thread.
  pub fn drive(&self) {
    DispatcherCore::drive(&self.core);
  }
}
