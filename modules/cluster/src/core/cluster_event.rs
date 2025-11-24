//! Cluster lifecycle and topology events emitted to the event stream.

use alloc::{string::String, vec::Vec};

use crate::core::startup_mode::StartupMode;

/// Event payload published via `EventStreamEvent::Extension { name: "cluster", .. }`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClusterEvent {
  /// Cluster startup succeeded.
  Startup {
    /// Advertised address.
    address: String,
    /// Startup mode.
    mode:    StartupMode,
  },
  /// Cluster startup failed.
  StartupFailed {
    /// Advertised address.
    address: String,
    /// Startup mode.
    mode:    StartupMode,
    /// Failure reason.
    reason:  String,
  },
  /// Cluster shutdown succeeded.
  Shutdown {
    /// Advertised address.
    address: String,
    /// Shutdown mode.
    mode:    StartupMode,
  },
  /// Cluster shutdown failed.
  ShutdownFailed {
    /// Advertised address.
    address: String,
    /// Shutdown mode.
    mode:    StartupMode,
    /// Failure reason.
    reason:  String,
  },
  /// Topology changed (joined/left/blocked members).
  Topology {
    /// Topology hash for change detection.
    topology_hash: u64,
    /// Joined members.
    joined:        Vec<String>,
    /// Left members.
    left:          Vec<String>,
    /// Blocked members from BlockListProvider.
    blocked:       Vec<String>,
  },
}
