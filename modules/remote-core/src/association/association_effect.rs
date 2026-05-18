//! Side-effects emitted by [`crate::association::Association`] state transitions.

use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_actor_core_kernel_rs::event::stream::RemotingLifecycleEvent;

use crate::{
  association::quarantine_reason::QuarantineReason,
  envelope::OutboundEnvelope,
  transport::TransportEndpoint,
  wire::{AckPdu, FlushScope},
};

/// Side-effect requested by an association state transition.
///
/// Transition methods on [`crate::association::Association`] always return a
/// contiguous container of effects (`Vec<AssociationEffect>`) — never a single
/// value — so the adapter can perform multiple actions for one state change
/// (e.g. `PublishLifecycle` + `SendEnvelopes`).
///
/// `PublishLifecycle` uses the pre-existing
/// `fraktor_actor_core_kernel_rs::event::stream::RemotingLifecycleEvent`
/// (see design Decision 16) rather than a duplicate in this crate.
#[derive(Debug)]
pub enum AssociationEffect {
  /// Kick off a handshake with the given authority.
  ///
  /// `Remote::run` is responsible for sending the handshake request through
  /// `RemoteTransport::send` and then calling
  /// `RemoteTransport::schedule_handshake_timeout`. Adapter code is
  /// responsible for scheduling a generation-scoped timer that sends
  /// `RemoteEvent::HandshakeTimerFired` back to the event receiver.
  StartHandshake {
    /// Endpoint against which the handshake should be performed.
    authority:  TransportEndpoint,
    /// Timeout to use when scheduling the handshake timer.
    timeout:    Duration,
    /// Generation active when this handshake was started.
    generation: u64,
  },
  /// Send the given envelopes to the remote peer (flushed from the deferred
  /// queue after a handshake completed).
  SendEnvelopes {
    /// Envelopes to send, in priority order.
    envelopes: Vec<OutboundEnvelope>,
  },
  /// Send an ACK/NACK PDU for inbound system-priority envelopes.
  SendAck {
    /// ACK/NACK PDU to send to the association peer.
    pdu: AckPdu,
  },
  /// Schedule a flush timeout outside the association state machine.
  ScheduleFlushTimeout {
    /// Endpoint whose flush timer should be scheduled.
    authority:   TransportEndpoint,
    /// Flush session identifier.
    flush_id:    u64,
    /// Flush scope.
    scope:       FlushScope,
    /// Monotonic deadline in milliseconds.
    deadline_ms: u64,
  },
  /// Send a lane-targeted flush request to the association peer.
  SendFlushRequest {
    /// Endpoint that should receive the flush request.
    authority:     TransportEndpoint,
    /// Flush session identifier.
    flush_id:      u64,
    /// Flush scope.
    scope:         FlushScope,
    /// Target writer lane id.
    lane_id:       u32,
    /// Number of acknowledgements expected for this session.
    expected_acks: u32,
  },
  /// A flush session completed successfully.
  FlushCompleted {
    /// Endpoint associated with the completed session.
    authority: TransportEndpoint,
    /// Flush session identifier.
    flush_id:  u64,
    /// Flush scope.
    scope:     FlushScope,
  },
  /// A flush session timed out.
  FlushTimedOut {
    /// Endpoint associated with the timed-out session.
    authority:     TransportEndpoint,
    /// Flush session identifier.
    flush_id:      u64,
    /// Flush scope.
    scope:         FlushScope,
    /// Lanes that did not acknowledge the flush.
    pending_lanes: Vec<u32>,
  },
  /// A flush session failed before it could complete.
  FlushFailed {
    /// Endpoint associated with the failed session.
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
  /// Re-send retained system-priority envelopes without assigning new sequence
  /// numbers.
  ResendEnvelopes {
    /// Envelopes to re-send, keeping their existing redelivery sequence.
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
