//! Listener notified about backpressure state changes.

use fraktor_actor_rs::core::event_stream::BackpressureSignal;

/// Observer notified whenever transports emit backpressure signals.
pub trait RemotingBackpressureListener: Send + Sync + 'static {
  /// Called whenever a transport toggles backpressure for a specific authority.
  fn on_signal(&self, signal: BackpressureSignal, authority: &str);
}
