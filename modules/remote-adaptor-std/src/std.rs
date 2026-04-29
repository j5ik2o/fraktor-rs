//! Standard `std + tokio` adaptors for the fraktor remote runtime.
//!
//! This module implements the standard-library adapters used by
//! `fraktor-remote-core-rs` ports on top of `tokio`'s async runtime. The
//! decomposition follows Apache Pekko Artery
//! (see `openspec/changes/remote-redesign/design.md` for the full rationale):
//!
//! | Submodule | Purpose |
//! |---|---|
//! | `tcp_transport` | Pekko Artery TCP transport implementation built on `tokio::net` + `tokio_util::codec::Framed`. |
//! | `association` | `tokio` task group that drives the pure `Association` state machine with real I/O. |
//! | `watcher_actor` | Wraps the pure `WatcherState` in an actor-core actor and drives it with a tokio timer. |
//! | `provider` | `StdRemoteActorRefProvider` performing the loopback / remote dispatch per design Decision 3-C. |
//! | `extension_installer` | Actor-system extension registration for `remote-core`'s `Remote`. |

extern crate std;

#[cfg(test)]
mod tests;

pub mod association;
pub mod extension_installer;
pub mod provider;
pub mod tcp_transport;
pub mod watcher_actor;
