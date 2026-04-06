use fraktor_utils_rs::core::sync::ArcShared;

#[cfg(test)]
mod tests;

use super::dispatcher_core::DispatcherCore;

/// Shared reference for driving dispatcher execution across threads.
///
/// This type wraps `DispatcherCore` in an `ArcShared`, allowing multiple
/// threads to safely access and execute dispatcher batches.
pub struct DispatchShared {
  core: ArcShared<DispatcherCore>,
}

impl Clone for DispatchShared {
  fn clone(&self) -> Self {
    Self { core: self.core.clone() }
  }
}

impl DispatchShared {
  pub(crate) const fn new(core: ArcShared<DispatcherCore>) -> Self {
    Self { core }
  }

  /// Runs a dispatcher batch immediately on the current thread.
  pub fn drive(&self) {
    DispatcherCore::drive(&self.core);
  }
}
