//! Adapter that converts a closure into a backpressure listener.

use alloc::boxed::Box;

use fraktor_actor_rs::core::event_stream::{BackpressureSignal, CorrelationId};

use crate::core::remoting_backpressure_listener::RemotingBackpressureListener;

/// Adapter that converts a closure into a [`RemotingBackpressureListener`].
pub struct FnRemotingBackpressureListener<F>
where
  F: FnMut(BackpressureSignal, &str, CorrelationId) + Clone + Send + 'static, {
  inner: F,
}

impl<F> FnRemotingBackpressureListener<F>
where
  F: FnMut(BackpressureSignal, &str, CorrelationId) + Clone + Send + 'static,
{
  /// Creates a new listener from the provided closure.
  #[must_use]
  pub const fn new(inner: F) -> Self {
    Self { inner }
  }
}

impl<F> Clone for FnRemotingBackpressureListener<F>
where
  F: FnMut(BackpressureSignal, &str, CorrelationId) + Clone + Send + 'static,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<F> RemotingBackpressureListener for FnRemotingBackpressureListener<F>
where
  F: FnMut(BackpressureSignal, &str, CorrelationId) + Clone + Send + Sync + 'static,
{
  fn on_signal(&mut self, signal: BackpressureSignal, authority: &str, correlation_id: CorrelationId) {
    (self.inner)(signal, authority, correlation_id);
  }

  fn clone_box(&self) -> Box<dyn RemotingBackpressureListener> {
    Box::new(self.clone())
  }
}
