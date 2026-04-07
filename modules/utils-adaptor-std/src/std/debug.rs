//! Std-only debugging helpers for surgical instrumentation.
//!
//! This entire module is gated at the parent level via
//! `#[cfg(any(test, feature = "test-support"))]` so it never leaks into
//! production builds. It exists as drop-in replacements for production
//! primitives that add deadlock / re-entry detection during testing.

mod debug_spin_sync_mutex;
mod debug_spin_sync_mutex_guard;

pub use debug_spin_sync_mutex::DebugSpinSyncMutex;
pub use debug_spin_sync_mutex_guard::DebugSpinSyncMutexGuard;
