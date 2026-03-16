//! Reason for a coordinated shutdown.

extern crate std;

use alloc::string::String;
use core::fmt;

/// Reason for the coordinated shutdown.
///
/// Predefined reasons match Apache Pekko's `CoordinatedShutdown.Reason` hierarchy.
/// Custom reasons can be provided via [`CoordinatedShutdownReason::Custom`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CoordinatedShutdownReason {
  /// The reason for the shutdown was unknown.
  Unknown,
  /// The shutdown was initiated by `ActorSystem::terminate`.
  ActorSystemTerminate,
  /// The shutdown was initiated by a process signal (e.g. SIGTERM).
  ProcessSignal,
  /// The shutdown was initiated by cluster downing.
  ClusterDowning,
  /// The shutdown was initiated by cluster leaving.
  ClusterLeaving,
  /// The shutdown was initiated by a failure to join a seed node.
  ClusterJoinUnsuccessful,
  /// The shutdown was initiated by an incompatible cluster configuration.
  IncompatibleConfigurationDetected,
  /// A custom, application-defined reason.
  Custom(String),
}

impl fmt::Display for CoordinatedShutdownReason {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::Unknown => write!(f, "UnknownReason"),
      | Self::ActorSystemTerminate => write!(f, "ActorSystemTerminateReason"),
      | Self::ProcessSignal => write!(f, "ProcessSignalReason"),
      | Self::ClusterDowning => write!(f, "ClusterDowningReason"),
      | Self::ClusterLeaving => write!(f, "ClusterLeavingReason"),
      | Self::ClusterJoinUnsuccessful => write!(f, "ClusterJoinUnsuccessfulReason"),
      | Self::IncompatibleConfigurationDetected => write!(f, "IncompatibleConfigurationDetectedReason"),
      | Self::Custom(name) => write!(f, "Custom({name})"),
    }
  }
}
