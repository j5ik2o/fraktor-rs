//! Logical gossip transport handoff.

use alloc::{format, string::String};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{GossipEnvelope, GossipEnvelopeDispatchOutcome, GossipPayloadKind, GossipTransportHandoffError};

#[cfg(test)]
#[path = "gossip_transport_handoff_test.rs"]
mod tests;

/// Envelope plus target endpoint mapping preserved at the std transport boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipTransportHandoff {
  envelope:        GossipEnvelope,
  target_endpoint: String,
}

impl GossipTransportHandoff {
  /// Creates a handoff after validating the envelope deadline and target peer mapping.
  ///
  /// # Errors
  ///
  /// Returns an error when the deadline has expired or the target peer is unknown.
  pub fn try_new(
    envelope: GossipEnvelope,
    peers: &[UniqueAddress],
    now_tick: u64,
  ) -> Result<Self, GossipTransportHandoffError> {
    if let GossipEnvelopeDispatchOutcome::DeadlineExpired { deadline_tick, now_tick } =
      envelope.dispatch_outcome(now_tick)
    {
      return Err(GossipTransportHandoffError::DeadlineExpired { deadline_tick, now_tick });
    }
    if !peers.iter().any(|peer| peer == envelope.to()) {
      return Err(GossipTransportHandoffError::UnknownPeer { peer: envelope.to().clone() });
    }
    Ok(Self { target_endpoint: endpoint_for(envelope.to()), envelope })
  }

  /// Returns the envelope carried by this handoff.
  #[must_use]
  pub const fn envelope(&self) -> &GossipEnvelope {
    &self.envelope
  }

  /// Returns the sender identity.
  #[must_use]
  pub const fn from(&self) -> &UniqueAddress {
    self.envelope.from()
  }

  /// Returns the target identity.
  #[must_use]
  pub const fn to(&self) -> &UniqueAddress {
    self.envelope.to()
  }

  /// Returns the logical payload kind.
  #[must_use]
  pub const fn payload_kind(&self) -> GossipPayloadKind {
    self.envelope.payload_kind()
  }

  /// Returns the target transport endpoint.
  #[must_use]
  pub const fn target_endpoint(&self) -> &str {
    self.target_endpoint.as_str()
  }

  /// Decodes the logical payload kind tag used by handoff-oriented tests.
  ///
  /// # Errors
  ///
  /// Returns an error when `tag` is not a known logical payload kind.
  pub const fn payload_kind_from_tag(tag: u8) -> Result<GossipPayloadKind, GossipTransportHandoffError> {
    match tag {
      | 0 => Ok(GossipPayloadKind::Delta),
      | 1 => Ok(GossipPayloadKind::FullState),
      | 2 => Ok(GossipPayloadKind::SeenDigest),
      | 3 => Ok(GossipPayloadKind::HeartbeatRequest),
      | 4 => Ok(GossipPayloadKind::HeartbeatResponse),
      | 5 => Ok(GossipPayloadKind::CrossDcHeartbeat),
      | _ => Err(GossipTransportHandoffError::UnknownPayloadKind { tag }),
    }
  }
}

fn endpoint_for(identity: &UniqueAddress) -> String {
  let host = identity.address().host();
  let port = identity.address().port();
  if host.contains(':') && !host.starts_with('[') { format!("[{host}]:{port}") } else { format!("{host}:{port}") }
}
