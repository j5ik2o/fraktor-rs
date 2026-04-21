//! Factory abstraction for materializing invoke guards.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::InvokeGuard;

/// Builds a guard instance for actor message invocation.
pub trait InvokeGuardFactory: Send + Sync {
  /// Materializes a guard instance.
  fn build(&self) -> ArcShared<Box<dyn InvokeGuard>>;
}
