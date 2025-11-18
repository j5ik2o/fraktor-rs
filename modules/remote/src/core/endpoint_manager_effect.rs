//! Effects emitted after processing a command.

use alloc::{string::String, vec::Vec};

use fraktor_actor_rs::core::event_stream::RemotingLifecycleEvent;

use crate::core::{
  deferred_envelope::DeferredEnvelope, quarantine_reason::QuarantineReason, transport::TransportEndpoint,
};

/// Effects emitted after processing a command.
#[derive(Debug, PartialEq, Eq)]
pub enum EndpointManagerEffect {
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
