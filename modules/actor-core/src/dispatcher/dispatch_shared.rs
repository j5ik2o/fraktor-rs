use cellactor_utils_core_rs::sync::ArcShared;

use super::dispatcher_core::DispatcherCore;
use crate::RuntimeToolbox;

/// Shared reference for driving dispatcher execution across threads.
///
/// This type wraps `DispatcherCore` in an `ArcShared`, allowing multiple
/// threads to safely access and execute dispatcher batches.
#[derive(Clone)]
pub struct DispatchShared<TB: RuntimeToolbox + 'static> {
  core: ArcShared<DispatcherCore<TB>>,
}

impl<TB: RuntimeToolbox + 'static> DispatchShared<TB> {
  pub(super) const fn new(core: ArcShared<DispatcherCore<TB>>) -> Self {
    Self { core }
  }

  /// Runs a dispatcher batch immediately on the current thread.
  pub fn drive(&self) {
    DispatcherCore::drive(&self.core);
  }
}
