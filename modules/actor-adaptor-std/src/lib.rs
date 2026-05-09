#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unknown_lints)]

//! Standard adaptors for the fraktor actor runtime.

extern crate alloc;

/// Actor-specific standard-library bindings.
pub mod actor;
mod blocker;
/// Dispatch bindings for the standard toolbox.
pub mod dispatch;
/// Event bindings for the standard toolbox.
pub mod event;
/// Pattern bindings for the standard toolbox.
pub mod pattern;
/// Test-support helpers for actor systems (test-support feature only).
#[cfg(feature = "test-support")]
pub mod system;
/// Tick driver bindings for the standard toolbox.
pub mod tick_driver;
/// Time bindings for the standard toolbox.
pub mod time;

pub use blocker::StdBlocker;
