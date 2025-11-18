//! Hook invoked by transports when they need to signal throttling or release.

use fraktor_actor_rs::core::event_stream::{BackpressureSignal, CorrelationId};

/// Receives transport-level backpressure notifications.
pub trait TransportBackpressureHook: Send + Sync + 'static {
  /// Called whenever the transport requests throttling or resumes a remote authority.
  fn on_backpressure(&self, signal: BackpressureSignal, authority: &str, correlation_id: CorrelationId);
}
