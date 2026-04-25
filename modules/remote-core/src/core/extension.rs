//! Pure lifecycle and extension surface for the remote subsystem.
//!
//! This module replaces the god-object `RemotingControlHandle` from the
//! legacy `fraktor-remote-rs` crate. Only the pure pieces live here
//! (lifecycle state machine, error enum, event publisher, data snapshots,
//! and the `Remoting` port trait) — all transport wiring, watcher daemons,
//! and heartbeat plumbing stay in the `fraktor-remote-adaptor-std-rs` crate
//! per design Decision 5.
//!
//! Per design Decision 16, this module deliberately does **not** define a
//! new `RemotingLifecycleEvent`; the already-shipped
//! `fraktor_actor_core_rs::core::kernel::event::stream::RemotingLifecycleEvent`
//! is re-used everywhere.

#[cfg(test)]
mod tests;

mod event_publisher;
mod lifecycle_state;
mod remote_authority_snapshot;
mod remoting;
mod remoting_error;

pub use event_publisher::EventPublisher;
pub use lifecycle_state::RemotingLifecycleState;
pub use remote_authority_snapshot::RemoteAuthoritySnapshot;
pub use remoting::Remoting;
pub use remoting_error::RemotingError;
