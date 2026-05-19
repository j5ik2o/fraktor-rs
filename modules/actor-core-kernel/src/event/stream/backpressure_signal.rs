//! Backpressure level reported by remoting transports.

/// Signal emitted when transports request throttling or resume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackpressureSignal {
  /// Transport requests senders to throttle.
  Apply,
  /// Transport allows senders to resume normal pacing.
  Release,
}
