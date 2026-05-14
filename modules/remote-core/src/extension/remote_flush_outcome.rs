//! Observable outcome of a remote flush session.

use alloc::{string::String, vec::Vec};

use crate::{transport::TransportEndpoint, wire::FlushScope};

/// Observable outcome of a remote flush session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteFlushOutcome {
  /// All expected lane acknowledgements were observed.
  Completed {
    /// Remote authority associated with the flush session.
    authority: TransportEndpoint,
    /// Flush session identifier.
    flush_id:  u64,
    /// Flush scope.
    scope:     FlushScope,
  },
  /// The session reached its deadline before all lane acknowledgements arrived.
  TimedOut {
    /// Remote authority associated with the flush session.
    authority:     TransportEndpoint,
    /// Flush session identifier.
    flush_id:      u64,
    /// Flush scope.
    scope:         FlushScope,
    /// Lanes that did not acknowledge the flush.
    pending_lanes: Vec<u32>,
  },
  /// The session could not provide the requested ordering guarantee.
  Failed {
    /// Remote authority associated with the flush session.
    authority:     TransportEndpoint,
    /// Flush session identifier.
    flush_id:      u64,
    /// Flush scope.
    scope:         FlushScope,
    /// Lanes that did not acknowledge the flush.
    pending_lanes: Vec<u32>,
    /// Human-readable failure reason.
    reason:        String,
  },
}

impl RemoteFlushOutcome {
  /// Returns the remote authority associated with this outcome.
  #[must_use]
  pub const fn authority(&self) -> &TransportEndpoint {
    match self {
      | Self::Completed { authority, .. } | Self::TimedOut { authority, .. } | Self::Failed { authority, .. } => {
        authority
      },
    }
  }

  /// Returns the flush session identifier.
  #[must_use]
  pub const fn flush_id(&self) -> u64 {
    match self {
      | Self::Completed { flush_id, .. } | Self::TimedOut { flush_id, .. } | Self::Failed { flush_id, .. } => *flush_id,
    }
  }

  /// Returns the flush scope.
  #[must_use]
  pub const fn scope(&self) -> FlushScope {
    match self {
      | Self::Completed { scope, .. } | Self::TimedOut { scope, .. } | Self::Failed { scope, .. } => *scope,
    }
  }
}
