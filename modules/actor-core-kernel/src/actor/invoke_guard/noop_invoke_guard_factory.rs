//! Factory for the default no-op invoke guard.

use alloc::boxed::Box;

use fraktor_utils_core_rs::sync::ArcShared;

use super::{InvokeGuard, InvokeGuardFactory, NoopInvokeGuard};

/// Produces no-op invoke guards for default actor systems.
pub struct NoopInvokeGuardFactory {
  shared_guard: ArcShared<dyn InvokeGuard>,
}

impl NoopInvokeGuardFactory {
  /// Creates a no-op guard factory.
  #[must_use]
  pub fn new() -> Self {
    let shared_guard: ArcShared<dyn InvokeGuard> = ArcShared::new(NoopInvokeGuard::new());
    Self { shared_guard }
  }

  /// Returns a shared trait-object wrapper of this factory.
  #[must_use]
  pub fn shared() -> ArcShared<Box<dyn InvokeGuardFactory>> {
    ArcShared::new(Box::new(Self::new()) as Box<dyn InvokeGuardFactory>)
  }
}

impl Default for NoopInvokeGuardFactory {
  fn default() -> Self {
    Self::new()
  }
}

impl InvokeGuardFactory for NoopInvokeGuardFactory {
  fn build(&self) -> ArcShared<dyn InvokeGuard> {
    self.shared_guard.clone()
  }
}
