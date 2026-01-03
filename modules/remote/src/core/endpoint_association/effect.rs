//! Effects emitted after processing a command.

use alloc::{string::String, vec::Vec};

use fraktor_actor_rs::core::event::stream::RemotingLifecycleEvent;

use super::quarantine_reason::QuarantineReason;
use crate::core::{envelope::DeferredEnvelope, transport::TransportEndpoint};

/// Effects emitted after processing a command.
#[derive(Debug, PartialEq, Eq)]
pub enum EndpointAssociationEffect {
  /// Requests that a handshake frame be sent via the transport.
  StartHandshake {
    /// Authority that should start a handshake.
    authority: String,
    /// Endpoint to contact.
    endpoint:  TransportEndpoint,
  },
  /// Requests the consumer to deliver the provided envelopes.
  DeliverEnvelopes {
    /// Authority whose queue was flushed.
    authority: String,
    /// Envelopes to deliver in order.
    envelopes: Vec<DeferredEnvelope>,
  },
  /// Notifies that deferred envelopes were discarded due to quarantine.
  DiscardDeferred {
    /// Authority whose queue was discarded.
    authority: String,
    /// Reason associated with the discard.
    reason:    QuarantineReason,
    /// Envelopes that were dropped.
    envelopes: Vec<DeferredEnvelope>,
  },
  /// Requests the caller to publish a lifecycle event.
  Lifecycle(RemotingLifecycleEvent),
}
