//! Grain observability events emitted to EventStream.

use alloc::string::String;

use crate::core::{ClusterIdentity, GrainKey};

/// EventStream extension name used for grain events.
pub const GRAIN_EVENT_STREAM_NAME: &str = "cluster-grain";

/// Observability events emitted by Grain API flows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrainEvent {
  /// Grain call failed.
  CallFailed {
    /// Target identity.
    identity: ClusterIdentity,
    /// Failure reason.
    reason:   String,
  },
  /// Grain call timed out.
  CallTimedOut {
    /// Target identity.
    identity: ClusterIdentity,
  },
  /// Grain call retry executed.
  CallRetrying {
    /// Target identity.
    identity: ClusterIdentity,
    /// Retry attempt number (0-based).
    attempt:  u32,
  },
  /// Activation was created or reactivated.
  ActivationCreated {
    /// Target grain key.
    key: GrainKey,
    /// Activated PID string.
    pid: String,
  },
  /// Activation was passivated.
  ActivationPassivated {
    /// Target grain key.
    key: GrainKey,
  },
}
