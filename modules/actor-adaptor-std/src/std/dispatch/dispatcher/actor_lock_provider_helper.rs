//! Helper constructors for actor-system scoped lock providers.

#[cfg(test)]
mod tests;

use fraktor_actor_core_rs::core::kernel::system::lock_provider::{BuiltinSpinLockProvider, DebugSpinLockProvider};

/// Returns the std-environment default lock provider helper.
///
/// The current std helper intentionally reuses the built-in spin provider so
/// the actor system can stay `no_std`-centric while callers opt in through a
/// std-facing API surface.
#[must_use]
pub const fn std_actor_lock_provider() -> BuiltinSpinLockProvider {
  BuiltinSpinLockProvider::new()
}

/// Returns the debug lock provider helper that panics on hot-path contention.
#[must_use]
pub const fn debug_actor_lock_provider() -> DebugSpinLockProvider {
  DebugSpinLockProvider::new()
}
