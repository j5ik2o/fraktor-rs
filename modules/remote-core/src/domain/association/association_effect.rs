//! Side-effects emitted by [`crate::domain::association::Association`] state transitions.

use alloc::vec::Vec;

use fraktor_actor_core_rs::core::kernel::event::stream::RemotingLifecycleEvent;

use crate::domain::{
  association::quarantine_reason::QuarantineReason, envelope::OutboundEnvelope, transport::TransportEndpoint,
};

/// Side-effect requested by an association state transition.
///
/// Transition methods on [`crate::domain::association::Association`] always return a
/// contiguous container of effects (`Vec<AssociationEffect>`) — never a single
/// value — so the adapter can perform multiple actions for one state change
/// (e.g. `PublishLifecycle` + `SendEnvelopes`).
///
/// `PublishLifecycle` uses the pre-existing
/// `fraktor_actor_core_rs::core::kernel::event::stream::RemotingLifecycleEvent`
/// (see design Decision 16) rather than a duplicate in this crate.
#[derive(Debug)]
pub enum AssociationEffect {
  /// Kick off a handshake with the given endpoint.
  StartHandshake {
    /// Endpoint against which the handshake should be performed.
    endpoint: TransportEndpoint,
  },
  /// Send the given envelopes to the remote peer (flushed from the deferred
  /// queue after a handshake completed).
  SendEnvelopes {
    /// Envelopes to send, in priority order.
    envelopes: Vec<OutboundEnvelope>,
  },
  /// Discard the given envelopes because the peer is quarantined.
  DiscardEnvelopes {
    /// Reason the envelopes are being discarded.
    reason:    QuarantineReason,
    /// Envelopes being discarded.
    envelopes: Vec<OutboundEnvelope>,
  },
  /// Publish a remoting lifecycle event through the actor-core event stream.
  PublishLifecycle(RemotingLifecycleEvent),
}
