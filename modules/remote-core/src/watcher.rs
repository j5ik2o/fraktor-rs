//! Pure state portion of the remote watcher.
//!
//! Pekko's `RemoteWatcher` is an Akka actor (Scala, ~342 lines); this module
//! contains only the **state-transition logic** and data types. Actor
//! lifecycle, scheduling, and heartbeat I/O are pushed to the
//! `fraktor-remote-adaptor-std-rs` crate.

#[cfg(test)]
#[path = "watcher_test.rs"]
mod tests;

mod watcher_command;
mod watcher_effect;
mod watcher_state;

pub use watcher_command::WatcherCommand;
pub use watcher_effect::WatcherEffect;
pub use watcher_state::WatcherState;
