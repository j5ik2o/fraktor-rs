//! [`RemoteInstrument`] trait: observability hooks for outbound and inbound
//! traffic.

use fraktor_actor_core_kernel_rs::event::stream::CorrelationId;

use crate::core::{
  association::QuarantineReason,
  envelope::{InboundEnvelope, OutboundEnvelope},
  instrument::HandshakePhase,
  transport::{BackpressureSignal, TransportEndpoint},
};

/// Pluggable hook trait invoked by the remote pipeline for every outbound /
/// inbound envelope crossing the boundary.
///
/// Implementations are typically lightweight — e.g. a counter, a tracing span
/// emitter, or a [`crate::core::instrument::RemotingFlightRecorder`] wrapper. The
/// trait intentionally contains no `async` methods: instrumentation must be
/// able to operate in a fully synchronous `no_std` context.
pub trait RemoteInstrument {
  /// Called just before an outbound envelope is handed to the transport.
  fn on_send(&mut self, envelope: &OutboundEnvelope, now_ms: u64);

  /// Records an outbound envelope discarded after an unrecoverable send failure.
  fn record_dropped_envelope(&mut self, authority: &TransportEndpoint, envelope: &OutboundEnvelope, now_ms: u64);

  /// Called once an inbound envelope has been decoded and is about to be
  /// dispatched to the local recipient.
  fn on_receive(&mut self, envelope: &InboundEnvelope, now_ms: u64);

  /// Records a handshake lifecycle phase for `authority`.
  fn record_handshake(&mut self, authority: &TransportEndpoint, phase: HandshakePhase, now_ms: u64);

  /// Records that `authority` has been quarantined for `reason`.
  fn record_quarantine(&mut self, authority: &TransportEndpoint, reason: &QuarantineReason, now_ms: u64);

  /// Records a backpressure signal observed for `authority`.
  fn record_backpressure(
    &mut self,
    authority: &TransportEndpoint,
    signal: BackpressureSignal,
    correlation_id: CorrelationId,
    now_ms: u64,
  );
}
