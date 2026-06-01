//! Identity-aware logical gossip envelope.

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{GossipEnvelopeDispatchOutcome, GossipEnvelopeError, GossipPayloadKind, MembershipVersion};

#[cfg(test)]
#[path = "gossip_envelope_test.rs"]
mod tests;

/// Logical gossip payload envelope used before byte-level transport serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipEnvelope {
  from:               UniqueAddress,
  to:                 UniqueAddress,
  payload_kind:       GossipPayloadKind,
  membership_version: MembershipVersion,
  deadline_tick:      u64,
}

impl GossipEnvelope {
  /// Creates a new envelope after confirming both endpoint identities.
  ///
  /// # Errors
  ///
  /// Returns [`GossipEnvelopeError::UnconfirmedIdentity`] when either endpoint has UID `0`.
  pub fn try_new(
    from: UniqueAddress,
    to: UniqueAddress,
    payload_kind: GossipPayloadKind,
    membership_version: MembershipVersion,
    deadline_tick: u64,
  ) -> Result<Self, GossipEnvelopeError> {
    let from_unconfirmed = from.uid() == 0;
    let to_unconfirmed = to.uid() == 0;
    if from_unconfirmed || to_unconfirmed {
      return Err(GossipEnvelopeError::UnconfirmedIdentity { from: from_unconfirmed, to: to_unconfirmed });
    }

    Ok(Self { from, to, payload_kind, membership_version, deadline_tick })
  }

  /// Returns the confirmed sender identity.
  #[must_use]
  pub const fn from(&self) -> &UniqueAddress {
    &self.from
  }

  /// Returns the confirmed receiver identity.
  #[must_use]
  pub const fn to(&self) -> &UniqueAddress {
    &self.to
  }

  /// Returns the logical payload kind.
  #[must_use]
  pub const fn payload_kind(&self) -> GossipPayloadKind {
    self.payload_kind
  }

  /// Returns the membership version associated with this payload.
  #[must_use]
  pub const fn membership_version(&self) -> MembershipVersion {
    self.membership_version
  }

  /// Returns the dispatch deadline tick.
  #[must_use]
  pub const fn deadline_tick(&self) -> u64 {
    self.deadline_tick
  }

  /// Checks whether the envelope can still be dispatched at `now_tick`.
  #[must_use]
  pub const fn dispatch_outcome(&self, now_tick: u64) -> GossipEnvelopeDispatchOutcome {
    if now_tick > self.deadline_tick {
      GossipEnvelopeDispatchOutcome::DeadlineExpired { deadline_tick: self.deadline_tick, now_tick }
    } else {
      GossipEnvelopeDispatchOutcome::Ready
    }
  }
}
