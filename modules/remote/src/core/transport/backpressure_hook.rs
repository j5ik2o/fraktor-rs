//! Hook invoked by transports when they need to signal throttling or release.

use fraktor_actor_rs::core::event::stream::{BackpressureSignal, CorrelationId};

/// Receives transport-level backpressure notifications.
///
/// Implementations should be wrapped in
/// [`TransportBackpressureHookShared`](super::TransportBackpressureHookShared) for shared access
/// using `with_write`:
///
/// ```text
/// let hook = TransportBackpressureHookShared::new(boxed_hook);
/// hook.with_write(|h| h.on_backpressure(signal, authority, correlation_id));
/// ```
pub trait TransportBackpressureHook: Send + Sync + 'static {
  /// Called whenever the transport requests throttling or resumes a remote authority.
  fn on_backpressure(&mut self, signal: BackpressureSignal, authority: &str, correlation_id: CorrelationId);
}
