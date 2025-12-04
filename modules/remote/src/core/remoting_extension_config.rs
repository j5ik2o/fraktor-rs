//! Builder-style configuration for installing the remoting extension.

use alloc::{
  boxed::Box,
  string::{String, ToString},
  vec::Vec,
};

use fraktor_actor_rs::core::event_stream::{BackpressureSignal, CorrelationId};

use crate::core::{
  fn_remoting_backpressure_listener::FnRemotingBackpressureListener,
  remoting_backpressure_listener::RemotingBackpressureListener,
};

/// Declarative configuration applied when the remoting extension is installed.
pub struct RemotingExtensionConfig {
  canonical_host:           String,
  canonical_port:           Option<u16>,
  auto_start:               bool,
  transport_scheme:         String,
  backpressure_listeners:   Vec<Box<dyn RemotingBackpressureListener>>,
  flight_recorder_capacity: usize,
}

impl Clone for RemotingExtensionConfig {
  fn clone(&self) -> Self {
    let listeners = self.backpressure_listeners.iter().map(|listener| listener.clone_box()).collect();
    Self {
      canonical_host:           self.canonical_host.clone(),
      canonical_port:           self.canonical_port,
      auto_start:               self.auto_start,
      transport_scheme:         self.transport_scheme.clone(),
      backpressure_listeners:   listeners,
      flight_recorder_capacity: self.flight_recorder_capacity,
    }
  }
}

impl RemotingExtensionConfig {
  /// Creates a config with empty host/port (will be inherited from ActorSystem) and auto-start
  /// enabled.
  #[must_use]
  pub fn new() -> Self {
    Self {
      canonical_host:           String::new(),
      canonical_port:           None,
      auto_start:               true,
      transport_scheme:         "fraktor.loopback".to_string(),
      backpressure_listeners:   Vec::new(),
      flight_recorder_capacity: 128,
    }
  }

  /// Overrides the canonical host.
  #[must_use]
  pub fn with_canonical_host(mut self, host: impl Into<String>) -> Self {
    self.canonical_host = host.into();
    self
  }

  /// Overrides the canonical port.
  #[must_use]
  pub fn with_canonical_port(mut self, port: u16) -> Self {
    self.canonical_port = Some(port);
    self
  }

  /// Enables or disables automatic start during installation.
  #[must_use]
  pub fn with_auto_start(mut self, enabled: bool) -> Self {
    self.auto_start = enabled;
    self
  }

  /// Overrides the transport scheme used when resolving transports.
  #[must_use]
  pub fn with_transport_scheme(mut self, scheme: impl Into<String>) -> Self {
    self.transport_scheme = scheme.into();
    self
  }

  /// Registers a backpressure listener executed immediately after installation.
  #[must_use]
  pub fn with_backpressure_listener<F>(mut self, listener: F) -> Self
  where
    F: FnMut(BackpressureSignal, &str, CorrelationId) + Clone + Send + Sync + 'static, {
    let concrete = FnRemotingBackpressureListener::new(listener);
    self.backpressure_listeners.push(Box::new(concrete));
    self
  }

  /// Overrides the flight recorder capacity.
  #[must_use]
  pub fn with_flight_recorder_capacity(mut self, capacity: usize) -> Self {
    self.flight_recorder_capacity = capacity.max(1);
    self
  }

  /// Returns the configured canonical host.
  #[must_use]
  pub fn canonical_host(&self) -> &str {
    &self.canonical_host
  }

  /// Returns the configured canonical port.
  #[must_use]
  pub const fn canonical_port(&self) -> Option<u16> {
    self.canonical_port
  }

  /// Returns whether auto-start is enabled.
  #[must_use]
  pub const fn auto_start(&self) -> bool {
    self.auto_start
  }

  /// Returns the registered backpressure listeners.
  #[must_use]
  pub fn backpressure_listeners(&self) -> &[Box<dyn RemotingBackpressureListener>] {
    &self.backpressure_listeners
  }

  /// Returns the configured transport scheme.
  #[must_use]
  pub fn transport_scheme(&self) -> &str {
    &self.transport_scheme
  }

  /// Returns the configured flight recorder capacity.
  #[must_use]
  pub const fn flight_recorder_capacity(&self) -> usize {
    self.flight_recorder_capacity
  }
}

impl Default for RemotingExtensionConfig {
  fn default() -> Self {
    Self::new()
  }
}
