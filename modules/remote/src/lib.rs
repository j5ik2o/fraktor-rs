#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(all(not(test), not(feature = "std")), no_std)]
#![allow(clippy::module_inception)]

//! Remoting facilities for the fraktor actor runtime.

extern crate alloc;

/// Core remoting facilities.
pub mod core;
/// Standard library implementation.
#[cfg(feature = "std")]
pub mod std;

// pub use 禁止
