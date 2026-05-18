#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unknown_lints)]

//! Standard adaptors for the fraktor remote runtime.

extern crate alloc;

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;

mod association;
mod deployment;
pub mod extension_installer;
pub mod provider;
mod tokio_remote_event_receiver;
pub mod transport;
mod watcher;
