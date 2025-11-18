//! Observers notified when transports request backpressure adjustments.

use fraktor_actor_rs::core::event_stream::{BackpressureSignal, CorrelationId};

/// Listener invoked whenever backpressure is applied or released for a remote authority.
pub trait RemotingBackpressureListener: Send + Sync + 'static {
  /// Called with the latest backpressure signal, authority identifier, and correlation id.
  fn on_signal(&self, signal: BackpressureSignal, authority: &str, correlation_id: CorrelationId);
}
