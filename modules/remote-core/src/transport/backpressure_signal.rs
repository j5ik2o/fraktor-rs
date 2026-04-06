//! Backpressure signal propagated between the transport and the association
//! send queue.

/// Directive flowing from the transport layer into the association send queue
/// to throttle or resume user traffic.
///
/// `Apply` pauses the user queue until a matching `Release` is delivered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BackpressureSignal {
  /// Pause user traffic until a corresponding `Release` arrives.
  Apply,
  /// Resume user traffic after a previous `Apply`.
  Release,
}
