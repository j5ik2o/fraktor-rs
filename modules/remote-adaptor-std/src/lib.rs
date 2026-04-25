#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unknown_lints)]

//! Standard adaptors for the fraktor remote runtime.

extern crate alloc;

/// Standard library and tokio-backed remote adapters.
pub mod std;
