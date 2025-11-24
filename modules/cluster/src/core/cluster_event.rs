//! Cluster lifecycle and topology events emitted to the event stream.

use alloc::string::String;

/// Startup/shutdown mode of the cluster runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupMode {
  /// Member node mode.
  Member,
  /// Client node mode.
  Client,
}

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
}
