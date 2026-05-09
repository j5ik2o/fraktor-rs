//! No-op [`crate::core::instrument::RemoteInstrument`] implementation.

use fraktor_actor_core_rs::event::stream::CorrelationId;

use crate::core::{
  association::QuarantineReason,
  envelope::{InboundEnvelope, OutboundEnvelope},
  instrument::{HandshakePhase, RemoteInstrument},
  transport::{BackpressureSignal, TransportEndpoint},
};

/// Internal no-op instrument used by [`crate::core::extension::Remote::new`].
pub(crate) struct NoopInstrument;

impl RemoteInstrument for NoopInstrument {
  fn on_send(&mut self, _envelope: &OutboundEnvelope, _now_ms: u64) {}

  fn record_dropped_envelope(&mut self, _authority: &TransportEndpoint, _envelope: &OutboundEnvelope, _now_ms: u64) {}

  fn on_receive(&mut self, _envelope: &InboundEnvelope, _now_ms: u64) {}

  fn record_handshake(&mut self, _authority: &TransportEndpoint, _phase: HandshakePhase, _now_ms: u64) {}

  fn record_quarantine(&mut self, _authority: &TransportEndpoint, _reason: &QuarantineReason, _now_ms: u64) {}

  fn record_backpressure(
    &mut self,
    _authority: &TransportEndpoint,
    _signal: BackpressureSignal,
    _correlation_id: CorrelationId,
    _now_ms: u64,
  ) {
  }
}
