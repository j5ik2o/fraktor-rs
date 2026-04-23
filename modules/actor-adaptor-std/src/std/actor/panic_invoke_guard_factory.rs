//! Factory for std panic-catching invoke guards.

use fraktor_actor_core_rs::core::kernel::actor::invoke_guard::{InvokeGuard, InvokeGuardFactory};
use fraktor_utils_core_rs::core::sync::ArcShared;

use super::panic_invoke_guard::PanicInvokeGuard;

/// Produces `PanicInvokeGuard` instances for std-enabled actor systems.
pub struct PanicInvokeGuardFactory {
  shared_guard: ArcShared<dyn InvokeGuard>,
}

impl PanicInvokeGuardFactory {
  /// Creates a panic guard factory.
  #[must_use]
  pub fn new() -> Self {
    let shared_guard: ArcShared<dyn InvokeGuard> = ArcShared::new(PanicInvokeGuard::new());
    Self { shared_guard }
  }
}

impl Default for PanicInvokeGuardFactory {
  fn default() -> Self {
    Self::new()
  }
}

impl InvokeGuardFactory for PanicInvokeGuardFactory {
  fn build(&self) -> ArcShared<dyn InvokeGuard> {
    self.shared_guard.clone()
  }
}
