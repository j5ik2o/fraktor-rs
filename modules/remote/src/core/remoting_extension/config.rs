//! Builder-style configuration for installing the remoting extension.

use alloc::{
  boxed::Box,
  string::{String, ToString},
  sync::Arc,
  vec::Vec,
};
use core::time::Duration;

use fraktor_actor_rs::core::event::stream::{BackpressureSignal, CorrelationId};

use crate::core::{
  RemoteInstrument,
  backpressure::{FnRemotingBackpressureListener, RemotingBackpressureListener},
};

#[cfg(test)]
mod tests;

const MIN_HANDSHAKE_TIMEOUT: Duration = Duration::from_millis(1);
const DEFAULT_SHUTDOWN_FLUSH_TIMEOUT: Duration = Duration::from_secs(3);

/// Declarative configuration applied when the remoting extension is installed.
pub struct RemotingExtensionConfig {
  canonical_host:           String,
  canonical_port:           Option<u16>,
  auto_start:               bool,
  handshake_timeout:        Duration,
  shutdown_flush_timeout:   Duration,
  ack_send_window:          usize,
  ack_receive_window:       u64,
  transport_scheme:         String,
  backpressure_listeners:   Vec<Box<dyn RemotingBackpressureListener>>,
  remote_instruments:       Vec<Arc<dyn RemoteInstrument>>,
  flight_recorder_capacity: usize,
}

impl Clone for RemotingExtensionConfig {
  fn clone(&self) -> Self {
    let listeners = self.backpressure_listeners.iter().map(|listener| listener.clone_box()).collect();
    Self {
      canonical_host:           self.canonical_host.clone(),
      canonical_port:           self.canonical_port,
      auto_start:               self.auto_start,
      handshake_timeout:        self.handshake_timeout,
      shutdown_flush_timeout:   self.shutdown_flush_timeout,
      ack_send_window:          self.ack_send_window,
      ack_receive_window:       self.ack_receive_window,
      transport_scheme:         self.transport_scheme.clone(),
      backpressure_listeners:   listeners,
      remote_instruments:       self.remote_instruments.clone(),
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
      handshake_timeout:        Duration::from_secs(3),
      shutdown_flush_timeout:   DEFAULT_SHUTDOWN_FLUSH_TIMEOUT,
      ack_send_window:          128,
      ack_receive_window:       128,
      transport_scheme:         "fraktor.loopback".to_string(),
      backpressure_listeners:   Vec::new(),
      remote_instruments:       Vec::new(),
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

  /// Overrides the handshake timeout used while waiting for association completion.
  ///
  /// # Panics
  ///
  /// Panics when `timeout` is shorter than one millisecond.
  #[must_use]
  pub fn with_handshake_timeout(mut self, timeout: Duration) -> Self {
    assert!(timeout >= MIN_HANDSHAKE_TIMEOUT, "handshake timeout must be >= 1 millisecond");
    self.handshake_timeout = timeout;
    self
  }

  /// Overrides the timeout applied when flushing messages during graceful shutdown.
  ///
  /// Defaults to three seconds. This timeout is independent of the handshake timeout.
  ///
  /// # Panics
  ///
  /// Panics when `timeout` is shorter than one millisecond.
  #[must_use]
  pub fn with_shutdown_flush_timeout(mut self, timeout: Duration) -> Self {
    assert!(timeout >= MIN_HANDSHAKE_TIMEOUT, "shutdown flush timeout must be >= 1 millisecond");
    self.shutdown_flush_timeout = timeout;
    self
  }

  /// Overrides the outbound ack send window.
  ///
  /// # Panics
  ///
  /// Panics when `window` is zero.
  #[must_use]
  pub fn with_ack_send_window(mut self, window: usize) -> Self {
    assert!(window > 0, "ack send window must be > 0");
    self.ack_send_window = window;
    self
  }

  /// Overrides the inbound ack receive window.
  ///
  /// # Panics
  ///
  /// Panics when `window` is zero.
  #[must_use]
  pub fn with_ack_receive_window(mut self, window: u64) -> Self {
    assert!(window > 0, "ack receive window must be > 0");
    self.ack_receive_window = window;
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

  /// Registers a remoting instrument used by transport pipelines.
  #[must_use]
  pub fn with_remote_instrument(mut self, instrument: Arc<dyn RemoteInstrument>) -> Self {
    self.remote_instruments.push(instrument);
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

  /// Returns the configured handshake timeout.
  #[must_use]
  pub const fn handshake_timeout(&self) -> Duration {
    self.handshake_timeout
  }

  /// Returns the configured shutdown flush timeout.
  #[must_use]
  pub const fn shutdown_flush_timeout(&self) -> Duration {
    self.shutdown_flush_timeout
  }

  /// Returns the configured outbound ack send window.
  #[must_use]
  pub const fn ack_send_window(&self) -> usize {
    self.ack_send_window
  }

  /// Returns the configured inbound ack receive window.
  #[must_use]
  pub const fn ack_receive_window(&self) -> u64 {
    self.ack_receive_window
  }

  /// Returns the registered backpressure listeners.
  #[must_use]
  pub fn backpressure_listeners(&self) -> &[Box<dyn RemotingBackpressureListener>] {
    &self.backpressure_listeners
  }

  /// Returns the registered remoting instruments.
  #[must_use]
  pub fn remote_instruments(&self) -> &[Arc<dyn RemoteInstrument>] {
    &self.remote_instruments
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
