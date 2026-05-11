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
//! `fraktor_actor_core_kernel_rs::event::stream::RemotingLifecycleEvent`
//! is re-used everywhere.
//!
//! [`RemoteShared::run`] uses a [`RemoteEventReceiver`]-driven wake-on-event
//! contract. If shutdown happens while the shared run future is pending on the
//! receiver, the task still needs a receiver wake event such as
//! [`RemoteEvent::TransportShutdown`] before it can observe termination. This
//! differs from exclusive [`RemoteRunFuture`], which checks termination at the
//! head of its loop whenever it is polled and therefore completes immediately
//! when it is polled after shutdown.

#[cfg(test)]
#[path = "extension_test.rs"]
mod tests;

mod event_publisher;
mod lifecycle_state;
mod remote;
mod remote_actor_ref_resolve_cache_event;
mod remote_actor_ref_resolve_cache_outcome;
mod remote_authority_snapshot;
mod remote_event;
mod remote_event_receiver;
mod remote_run_future;
mod remote_shared;
mod remote_shared_run_future;
mod remoting;
mod remoting_error;

pub use event_publisher::EventPublisher;
pub use lifecycle_state::RemotingLifecycleState;
pub use remote::Remote;
pub use remote_actor_ref_resolve_cache_event::{
  REMOTE_ACTOR_REF_RESOLVE_CACHE_EXTENSION, RemoteActorRefResolveCacheEvent,
};
pub use remote_actor_ref_resolve_cache_outcome::RemoteActorRefResolveCacheOutcome;
pub use remote_authority_snapshot::RemoteAuthoritySnapshot;
pub use remote_event::RemoteEvent;
pub use remote_event_receiver::RemoteEventReceiver;
pub use remote_run_future::RemoteRunFuture;
pub use remote_shared::RemoteShared;
pub use remote_shared_run_future::RemoteSharedRunFuture;
pub use remoting::Remoting;
pub use remoting_error::RemotingError;
