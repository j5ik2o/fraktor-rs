#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unknown_lints)]

//! Std adapter helpers for `fraktor-utils-core-rs`.

extern crate alloc;

/// Standard-library-backed lock drivers.
pub mod std;
