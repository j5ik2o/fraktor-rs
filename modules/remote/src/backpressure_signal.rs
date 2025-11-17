//! Backpressure signal emitted by transports.

/// Indicates whether a transport requests throttling or resume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackpressureSignal {
  /// Transport requests the sender to slow down or pause.
  Apply,
  /// Transport notifies that normal traffic can resume.
  Release,
}
