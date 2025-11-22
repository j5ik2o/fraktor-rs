//! Events emitted by virtual actor registry.

use alloc::string::String;

use crate::core::grain_key::GrainKey;

#[cfg(test)]
mod tests;

/// Observable events for activation lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualActorEvent {
  /// Fresh activation created.
  Activated {
    /// Grain key.
    key:       GrainKey,
    /// Assigned PID string.
    pid:       String,
    /// Hosting authority.
    authority: String,
  },
  /// Activation reused (cache hit).
  Hit {
    /// Grain key.
    key: GrainKey,
    /// Cached PID.
    pid: String,
  },
  /// Activation moved to a new authority.
  Reactivated {
    /// Grain key.
    key:       GrainKey,
    /// New PID.
    pid:       String,
    /// New authority.
    authority: String,
  },
  /// Activation was passivated due to idleness.
  Passivated {
    /// Grain key.
    key: GrainKey,
  },
  /// Activation was dropped because snapshot missing.
  SnapshotMissing {
    /// Grain key.
    key: GrainKey,
  },
}
