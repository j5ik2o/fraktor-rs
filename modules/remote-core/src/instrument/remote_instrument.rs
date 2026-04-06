//! [`RemoteInstrument`] trait: observability hooks for outbound and inbound
//! traffic.

use crate::envelope::{InboundEnvelope, OutboundEnvelope};

/// Pluggable hook trait invoked by the remote pipeline for every outbound /
/// inbound envelope crossing the boundary.
///
/// Implementations are typically lightweight — e.g. a counter, a tracing span
/// emitter, or a [`crate::instrument::RemotingFlightRecorder`] wrapper. The
/// trait intentionally contains no `async` methods: instrumentation must be
/// able to operate in a fully synchronous `no_std` context.
pub trait RemoteInstrument {
  /// Called just before an outbound envelope is handed to the transport.
  fn on_send(&mut self, envelope: &OutboundEnvelope);

  /// Called once an inbound envelope has been decoded and is about to be
  /// dispatched to the local recipient.
  fn on_receive(&mut self, envelope: &InboundEnvelope);
}
