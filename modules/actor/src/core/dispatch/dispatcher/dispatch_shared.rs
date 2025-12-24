use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

#[cfg(test)]
mod tests;

use super::dispatcher_core::DispatcherCore;

/// Shared reference for driving dispatcher execution across threads.
///
/// This type wraps `DispatcherCore` in an `ArcShared`, allowing multiple
/// threads to safely access and execute dispatcher batches.
pub struct DispatchSharedGeneric<TB: RuntimeToolbox + 'static> {
  core: ArcShared<DispatcherCore<TB>>,
}

/// Type alias for `DispatchShared` with the default `NoStdToolbox`.
pub type DispatchShared = DispatchSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> Clone for DispatchSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { core: self.core.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> DispatchSharedGeneric<TB> {
  pub(crate) const fn new(core: ArcShared<DispatcherCore<TB>>) -> Self {
    Self { core }
  }

  /// Runs a dispatcher batch immediately on the current thread.
  pub fn drive(&self) {
    DispatcherCore::drive(&self.core);
  }
}
