//! Correlation trace hop enumeration.

/// Enumerates trace hop kinds recorded by the flight recorder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CorrelationTraceHop {
  /// Outbound send path.
  Send,
  /// Inbound receive path.
  Receive,
  /// Serialization hop.
  Serialize,
}
