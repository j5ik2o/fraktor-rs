//! Std adapter helpers for `fraktor-utils-core-rs`.
//!
//! This crate hosts std-dependent test/debug instrumentation that complements
//! the no_std core utilities. Production code should depend on
//! `fraktor-utils-core-rs` directly; tests and debug builds may pull in this
//! crate via the `test-support` feature for additional helpers such as
//! `std::debug::DebugSpinSyncMutex`.

/// Std-only adapter modules.
#[cfg(any(test, feature = "test-support"))]
pub mod std;
