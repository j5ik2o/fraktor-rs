//! Finite state machine states of an [`crate::association::Association`].

use crate::{address::RemoteNodeId, association::quarantine_reason::QuarantineReason, transport::TransportEndpoint};

/// Possible states of an [`crate::association::Association`].
///
/// All timestamps are **monotonic millis** (see design Decision 7). The state
/// machine never reads the current time itself — callers pass it as an
/// argument to each transition method.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssociationState {
  /// No connection attempted yet.
  Idle,
  /// A handshake is in flight.
  Handshaking {
    /// Endpoint the handshake is targeting.
    endpoint:   TransportEndpoint,
    /// `now_ms` passed to the `associate` call that started this handshake.
    started_at: u64,
  },
  /// Handshake completed and the association is live.
  Active {
    /// Remote node identity learned during handshake.
    remote_node:    RemoteNodeId,
    /// `now_ms` passed to `handshake_accepted`.
    established_at: u64,
    /// Last monotonic millis at which handshake activity was observed.
    last_used_at:   u64,
  },
  /// Temporarily gated after a transient failure.
  Gated {
    /// Monotonic millis at which the gate should lift, if known.
    resume_at: Option<u64>,
  },
  /// The peer has been quarantined and must be re-associated explicitly.
  Quarantined {
    /// Reason recorded at quarantine time.
    reason:    QuarantineReason,
    /// Monotonic millis at which the quarantined association may be removed, if known.
    resume_at: Option<u64>,
  },
}

impl AssociationState {
  /// Returns `true` when the state represents a live association.
  #[must_use]
  pub const fn is_active(&self) -> bool {
    matches!(self, AssociationState::Active { .. })
  }

  /// Returns `true` when the state is either `Handshaking` or `Active`
  /// (connection in progress or complete).
  #[must_use]
  pub const fn is_connected(&self) -> bool {
    matches!(self, AssociationState::Handshaking { .. } | AssociationState::Active { .. })
  }

  /// Returns `true` when the state is `Quarantined`.
  #[must_use]
  pub const fn is_quarantined(&self) -> bool {
    matches!(self, AssociationState::Quarantined { .. })
  }

  /// Returns `true` when the state is `Gated`.
  #[must_use]
  pub const fn is_gated(&self) -> bool {
    matches!(self, AssociationState::Gated { .. })
  }

  /// Returns `true` when the state is `Idle`.
  #[must_use]
  pub const fn is_idle(&self) -> bool {
    matches!(self, AssociationState::Idle)
  }
}
