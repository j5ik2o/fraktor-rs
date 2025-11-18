//! Adapter that converts a closure into a backpressure listener.

use fraktor_actor_rs::core::event_stream::{BackpressureSignal, CorrelationId};

use crate::core::remoting_backpressure_listener::RemotingBackpressureListener;

/// Adapter that converts a closure into a [`RemotingBackpressureListener`].
pub struct FnRemotingBackpressureListener<F>
where
  F: Fn(BackpressureSignal, &str, CorrelationId) + Send + Sync + 'static, {
  inner: F,
}

impl<F> FnRemotingBackpressureListener<F>
where
  F: Fn(BackpressureSignal, &str, CorrelationId) + Send + Sync + 'static,
{
  /// Creates a new listener from the provided closure.
  #[must_use]
  pub const fn new(inner: F) -> Self {
    Self { inner }
  }
}

impl<F> RemotingBackpressureListener for FnRemotingBackpressureListener<F>
where
  F: Fn(BackpressureSignal, &str, CorrelationId) + Send + Sync + 'static,
{
  fn on_signal(&self, signal: BackpressureSignal, authority: &str, correlation_id: CorrelationId) {
    (self.inner)(signal, authority, correlation_id);
  }
}
