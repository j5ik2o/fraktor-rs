//! Typed `RemoteSettings` with a `self`-consuming builder API.

use alloc::string::String;
use core::time::Duration;

/// Default handshake timeout (15 seconds), matching Pekko Artery defaults.
const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);

/// Default shutdown flush timeout (5 seconds).
const DEFAULT_SHUTDOWN_FLUSH_TIMEOUT: Duration = Duration::from_secs(5);

/// Default flight recorder ring buffer capacity.
const DEFAULT_FLIGHT_RECORDER_CAPACITY: usize = 1024;

/// Typed remote subsystem configuration.
///
/// Modeled after Pekko Artery's `RemoteSettings`, but expressed as a pure Rust
/// struct with a `self`-consuming builder API (see Decision 11). Only the fields
/// required by Phase A are declared here; `ack_send_window` / `ack_receive_window`
/// will be added in Phase B together with the ack-based redelivery runtime in the
/// `std` adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteSettings {
  canonical_host:           String,
  canonical_port:           Option<u16>,
  handshake_timeout:        Duration,
  shutdown_flush_timeout:   Duration,
  flight_recorder_capacity: usize,
}

impl RemoteSettings {
  /// Creates a new [`RemoteSettings`] with the given canonical host and default values
  /// for every other field.
  #[must_use]
  pub fn new(canonical_host: impl Into<String>) -> Self {
    Self {
      canonical_host:           canonical_host.into(),
      canonical_port:           None,
      handshake_timeout:        DEFAULT_HANDSHAKE_TIMEOUT,
      shutdown_flush_timeout:   DEFAULT_SHUTDOWN_FLUSH_TIMEOUT,
      flight_recorder_capacity: DEFAULT_FLIGHT_RECORDER_CAPACITY,
    }
  }

  /// Returns a copy with the given canonical port.
  #[must_use]
  pub const fn with_canonical_port(mut self, port: u16) -> Self {
    self.canonical_port = Some(port);
    self
  }

  /// Returns a copy with the given handshake timeout.
  #[must_use]
  pub const fn with_handshake_timeout(mut self, timeout: Duration) -> Self {
    self.handshake_timeout = timeout;
    self
  }

  /// Returns a copy with the given shutdown flush timeout.
  #[must_use]
  pub const fn with_shutdown_flush_timeout(mut self, timeout: Duration) -> Self {
    self.shutdown_flush_timeout = timeout;
    self
  }

  /// Returns a copy with the given flight recorder capacity.
  #[must_use]
  pub const fn with_flight_recorder_capacity(mut self, capacity: usize) -> Self {
    self.flight_recorder_capacity = capacity;
    self
  }

  /// Returns the canonical host name.
  #[must_use]
  pub fn canonical_host(&self) -> &str {
    &self.canonical_host
  }

  /// Returns the canonical port, if configured.
  #[must_use]
  pub const fn canonical_port(&self) -> Option<u16> {
    self.canonical_port
  }

  /// Returns the handshake timeout.
  #[must_use]
  pub const fn handshake_timeout(&self) -> Duration {
    self.handshake_timeout
  }

  /// Returns the shutdown flush timeout.
  #[must_use]
  pub const fn shutdown_flush_timeout(&self) -> Duration {
    self.shutdown_flush_timeout
  }

  /// Returns the flight recorder ring buffer capacity.
  #[must_use]
  pub const fn flight_recorder_capacity(&self) -> usize {
    self.flight_recorder_capacity
  }
}
