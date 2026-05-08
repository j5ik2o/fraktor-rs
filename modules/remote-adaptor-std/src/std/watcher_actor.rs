//! Tokio-based actor wrapping the pure
//! [`fraktor_remote_core_rs::core::watcher::WatcherState`].
//!
//! `remote-core` keeps the watcher logic completely actor- and
//! runtime-independent: it is just a `&mut self` state machine that consumes
//! [`fraktor_remote_core_rs::core::watcher::WatcherCommand`] and returns
//! [`fraktor_remote_core_rs::core::watcher::WatcherEffect`] values. This module
//! re-introduces the actor / scheduler concerns (per design Decision 9):
//!
//! - [`base::WatcherActor`] owns the `WatcherState`, exposes a
//!   `mpsc::UnboundedSender<WatcherCommand>` for callers, and runs the state machine inside a
//!   single tokio task so the `&mut self` contract holds without any extra locking.
//! - [`heartbeat_loop::run_heartbeat_loop`] is a sibling tokio task that ticks at a configurable
//!   interval, deriving `now_ms` from `Instant::now().elapsed()` (monotonic) and submitting
//!   `WatcherCommand::HeartbeatTick { now }` to the actor.

#[cfg(test)]
mod tests;

mod base;
mod heartbeat_loop;
mod watcher_actor_handle;
