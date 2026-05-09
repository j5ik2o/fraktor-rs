//! Factory abstraction for materializing invoke guards.

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::InvokeGuard;

/// Builds a guard instance for actor message invocation.
pub trait InvokeGuardFactory: Send + Sync {
  /// Returns a shared guard instance.
  fn build(&self) -> ArcShared<dyn InvokeGuard>;
}
