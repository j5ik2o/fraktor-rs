//! Tokio-side helpers for the remote adapter.
//!
//! Association state transitions now live in `remote-core`'s `Remote` event
//! loop. The std adapter keeps only I/O workers, time conversion helpers, and
//! small data structures that are still adapter-local.

mod inbound_dispatch;
mod monotonic_millis;

pub use inbound_dispatch::run_inbound_dispatch;
pub(crate) use monotonic_millis::std_instant_elapsed_millis;
