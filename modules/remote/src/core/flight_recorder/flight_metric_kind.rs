//! Kind of metric recorded by the flight recorder.

use fraktor_actor_rs::core::event_stream::BackpressureSignal;

/// Kind of metric recorded by the flight recorder.
#[derive(Clone, Debug, PartialEq)]
pub enum FlightMetricKind {
  /// Indicates a backpressure signal.
  Backpressure(BackpressureSignal),
  /// Authority was marked suspect by the failure detector.
  Suspect {
    /// Phi value emitted when the authority became suspect.
    phi: f64,
  },
  /// Authority recovered after being suspect.
  Reachable,
}
