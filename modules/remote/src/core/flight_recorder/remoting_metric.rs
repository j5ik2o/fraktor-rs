//! Remoting metric samples stored inside the flight recorder.

use alloc::string::String;

use fraktor_actor_rs::core::event_stream::BackpressureSignal;

/// Snapshot describing a remoting metric.
#[derive(Clone, Debug, PartialEq)]
pub struct RemotingMetric {
  authority:      String,
  latency_ms:     u32,
  deferred_depth: u16,
  backpressure:   Option<BackpressureSignal>,
  last_error:     Option<String>,
}

impl RemotingMetric {
  /// Creates a new metric.
  #[must_use]
  pub fn new(authority: impl Into<String>) -> Self {
    Self {
      authority:      authority.into(),
      latency_ms:     0,
      deferred_depth: 0,
      backpressure:   None,
      last_error:     None,
    }
  }

  /// Sets latency component.
  #[must_use]
  pub const fn with_latency_ms(mut self, latency_ms: u32) -> Self {
    self.latency_ms = latency_ms;
    self
  }

  /// Sets deferred queue depth.
  #[must_use]
  pub const fn with_deferred_depth(mut self, depth: u16) -> Self {
    self.deferred_depth = depth;
    self
  }

  /// Sets backpressure signal.
  #[must_use]
  pub fn with_backpressure(mut self, signal: Option<BackpressureSignal>) -> Self {
    self.backpressure = signal;
    self
  }

  /// Sets last error.
  #[must_use]
  pub fn with_last_error(mut self, error: Option<String>) -> Self {
    self.last_error = error;
    self
  }

  /// Returns authority identifier.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }

  /// Returns recorded latency in milliseconds.
  #[must_use]
  pub const fn latency_ms(&self) -> u32 {
    self.latency_ms
  }

  /// Returns deferred queue depth.
  #[must_use]
  pub const fn deferred_depth(&self) -> u16 {
    self.deferred_depth
  }

  /// Returns backpressure signal if captured.
  #[must_use]
  pub const fn backpressure(&self) -> Option<BackpressureSignal> {
    self.backpressure
  }

  /// Returns last error description if recorded.
  #[must_use]
  pub fn last_error(&self) -> Option<&str> {
    self.last_error.as_deref()
  }
}
