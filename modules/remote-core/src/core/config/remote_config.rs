//! Typed `RemoteConfig` with a `self`-consuming builder API.

use alloc::string::String;
use core::time::Duration;

/// Default handshake timeout (20 seconds), matching Pekko Artery advanced defaults.
const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(20);

/// Default shutdown flush timeout (5 seconds).
const DEFAULT_SHUTDOWN_FLUSH_TIMEOUT: Duration = Duration::from_secs(5);

/// Default flight recorder ring buffer capacity.
const DEFAULT_FLIGHT_RECORDER_CAPACITY: usize = 1024;

/// Default ack-based redelivery send window (Pekko Artery default).
const DEFAULT_ACK_SEND_WINDOW: u32 = 1024;

/// Default ack-based redelivery receive window (Pekko Artery default).
const DEFAULT_ACK_RECEIVE_WINDOW: u32 = 1024;

/// Default system message buffer size.
const DEFAULT_SYSTEM_MESSAGE_BUFFER_SIZE: usize = 20_000;

/// Default system message resend interval.
const DEFAULT_SYSTEM_MESSAGE_RESEND_INTERVAL: Duration = Duration::from_secs(1);

/// Default duration before giving up on unacknowledged system messages.
const DEFAULT_GIVE_UP_SYSTEM_MESSAGE_AFTER: Duration = Duration::from_secs(6 * 60 * 60);

/// Default handshake retry interval.
const DEFAULT_HANDSHAKE_RETRY_INTERVAL: Duration = Duration::from_secs(1);

/// Default periodic handshake injection interval.
const DEFAULT_INJECT_HANDSHAKE_INTERVAL: Duration = Duration::from_secs(1);

/// Default idle timeout for outbound streams.
const DEFAULT_STOP_IDLE_OUTBOUND_AFTER: Duration = Duration::from_secs(5 * 60);

/// Default quarantine timeout for idle outbound streams.
const DEFAULT_QUARANTINE_IDLE_OUTBOUND_AFTER: Duration = Duration::from_secs(6 * 60 * 60);

/// Default idle timeout for stopping quarantined outbound streams.
const DEFAULT_STOP_QUARANTINED_AFTER_IDLE: Duration = Duration::from_secs(3);

/// Default outbound stream restart backoff.
const DEFAULT_OUTBOUND_RESTART_BACKOFF: Duration = Duration::from_secs(1);

/// Default outbound stream restart timeout.
const DEFAULT_OUTBOUND_RESTART_TIMEOUT: Duration = Duration::from_secs(5);

/// Default maximum outbound stream restart count.
const DEFAULT_OUTBOUND_MAX_RESTARTS: u32 = 5;

/// Typed remote subsystem configuration.
///
/// Modeled after Pekko Artery's `RemoteSettings` (`RemoteConfig` in fraktor-rs), expressed as a
/// pure Rust struct with a `self`-consuming builder API (see Decision 11). The
/// ack-based redelivery and Artery advanced timing fields provide the core contract used by the
/// `std` adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteConfig {
  canonical_host:                 String,
  canonical_port:                 Option<u16>,
  handshake_timeout:              Duration,
  shutdown_flush_timeout:         Duration,
  flight_recorder_capacity:       usize,
  ack_send_window:                u32,
  ack_receive_window:             u32,
  system_message_buffer_size:     usize,
  system_message_resend_interval: Duration,
  give_up_system_message_after:   Duration,
  handshake_retry_interval:       Duration,
  inject_handshake_interval:      Duration,
  stop_idle_outbound_after:       Duration,
  quarantine_idle_outbound_after: Duration,
  stop_quarantined_after_idle:    Duration,
  outbound_restart_backoff:       Duration,
  outbound_restart_timeout:       Duration,
  outbound_max_restarts:          u32,
}

impl RemoteConfig {
  /// Creates a new [`RemoteConfig`] with the given canonical host and default values
  /// for every other field.
  #[must_use]
  pub fn new(canonical_host: impl Into<String>) -> Self {
    Self {
      canonical_host:                 canonical_host.into(),
      canonical_port:                 None,
      handshake_timeout:              DEFAULT_HANDSHAKE_TIMEOUT,
      shutdown_flush_timeout:         DEFAULT_SHUTDOWN_FLUSH_TIMEOUT,
      flight_recorder_capacity:       DEFAULT_FLIGHT_RECORDER_CAPACITY,
      ack_send_window:                DEFAULT_ACK_SEND_WINDOW,
      ack_receive_window:             DEFAULT_ACK_RECEIVE_WINDOW,
      system_message_buffer_size:     DEFAULT_SYSTEM_MESSAGE_BUFFER_SIZE,
      system_message_resend_interval: DEFAULT_SYSTEM_MESSAGE_RESEND_INTERVAL,
      give_up_system_message_after:   DEFAULT_GIVE_UP_SYSTEM_MESSAGE_AFTER,
      handshake_retry_interval:       DEFAULT_HANDSHAKE_RETRY_INTERVAL,
      inject_handshake_interval:      DEFAULT_INJECT_HANDSHAKE_INTERVAL,
      stop_idle_outbound_after:       DEFAULT_STOP_IDLE_OUTBOUND_AFTER,
      quarantine_idle_outbound_after: DEFAULT_QUARANTINE_IDLE_OUTBOUND_AFTER,
      stop_quarantined_after_idle:    DEFAULT_STOP_QUARANTINED_AFTER_IDLE,
      outbound_restart_backoff:       DEFAULT_OUTBOUND_RESTART_BACKOFF,
      outbound_restart_timeout:       DEFAULT_OUTBOUND_RESTART_TIMEOUT,
      outbound_max_restarts:          DEFAULT_OUTBOUND_MAX_RESTARTS,
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

  /// Returns a copy with the given ack send window.
  #[must_use]
  pub const fn with_ack_send_window(mut self, window: u32) -> Self {
    self.ack_send_window = window;
    self
  }

  /// Returns a copy with the given ack receive window.
  #[must_use]
  pub const fn with_ack_receive_window(mut self, window: u32) -> Self {
    self.ack_receive_window = window;
    self
  }

  /// Returns a copy with the given system message buffer size.
  #[must_use]
  pub const fn with_system_message_buffer_size(mut self, size: usize) -> Self {
    self.system_message_buffer_size = size;
    self
  }

  /// Returns a copy with the given system message resend interval.
  #[must_use]
  pub const fn with_system_message_resend_interval(mut self, interval: Duration) -> Self {
    self.system_message_resend_interval = interval;
    self
  }

  /// Returns a copy with the given duration before giving up on system messages.
  #[must_use]
  pub const fn with_give_up_system_message_after(mut self, duration: Duration) -> Self {
    self.give_up_system_message_after = duration;
    self
  }

  /// Returns a copy with the given handshake retry interval.
  #[must_use]
  pub const fn with_handshake_retry_interval(mut self, interval: Duration) -> Self {
    self.handshake_retry_interval = interval;
    self
  }

  /// Returns a copy with the given periodic handshake injection interval.
  #[must_use]
  pub const fn with_inject_handshake_interval(mut self, interval: Duration) -> Self {
    self.inject_handshake_interval = interval;
    self
  }

  /// Returns a copy with the given idle timeout for outbound streams.
  #[must_use]
  pub const fn with_stop_idle_outbound_after(mut self, duration: Duration) -> Self {
    self.stop_idle_outbound_after = duration;
    self
  }

  /// Returns a copy with the given quarantine timeout for idle outbound streams.
  #[must_use]
  pub const fn with_quarantine_idle_outbound_after(mut self, duration: Duration) -> Self {
    self.quarantine_idle_outbound_after = duration;
    self
  }

  /// Returns a copy with the given idle timeout for quarantined outbound streams.
  #[must_use]
  pub const fn with_stop_quarantined_after_idle(mut self, duration: Duration) -> Self {
    self.stop_quarantined_after_idle = duration;
    self
  }

  /// Returns a copy with the given outbound stream restart backoff.
  #[must_use]
  pub const fn with_outbound_restart_backoff(mut self, duration: Duration) -> Self {
    self.outbound_restart_backoff = duration;
    self
  }

  /// Returns a copy with the given outbound stream restart timeout.
  #[must_use]
  pub const fn with_outbound_restart_timeout(mut self, duration: Duration) -> Self {
    self.outbound_restart_timeout = duration;
    self
  }

  /// Returns a copy with the given maximum outbound stream restart count.
  #[must_use]
  pub const fn with_outbound_max_restarts(mut self, max_restarts: u32) -> Self {
    self.outbound_max_restarts = max_restarts;
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

  /// Returns the ack send window.
  #[must_use]
  pub const fn ack_send_window(&self) -> u32 {
    self.ack_send_window
  }

  /// Returns the ack receive window.
  #[must_use]
  pub const fn ack_receive_window(&self) -> u32 {
    self.ack_receive_window
  }

  /// Returns the system message buffer size.
  #[must_use]
  pub const fn system_message_buffer_size(&self) -> usize {
    self.system_message_buffer_size
  }

  /// Returns the system message resend interval.
  #[must_use]
  pub const fn system_message_resend_interval(&self) -> Duration {
    self.system_message_resend_interval
  }

  /// Returns the duration before giving up on unacknowledged system messages.
  #[must_use]
  pub const fn give_up_system_message_after(&self) -> Duration {
    self.give_up_system_message_after
  }

  /// Returns the handshake retry interval.
  #[must_use]
  pub const fn handshake_retry_interval(&self) -> Duration {
    self.handshake_retry_interval
  }

  /// Returns the periodic handshake injection interval.
  #[must_use]
  pub const fn inject_handshake_interval(&self) -> Duration {
    self.inject_handshake_interval
  }

  /// Returns the idle timeout for outbound streams.
  #[must_use]
  pub const fn stop_idle_outbound_after(&self) -> Duration {
    self.stop_idle_outbound_after
  }

  /// Returns the quarantine timeout for idle outbound streams.
  #[must_use]
  pub const fn quarantine_idle_outbound_after(&self) -> Duration {
    self.quarantine_idle_outbound_after
  }

  /// Returns the idle timeout for quarantined outbound streams.
  #[must_use]
  pub const fn stop_quarantined_after_idle(&self) -> Duration {
    self.stop_quarantined_after_idle
  }

  /// Returns the outbound stream restart backoff.
  #[must_use]
  pub const fn outbound_restart_backoff(&self) -> Duration {
    self.outbound_restart_backoff
  }

  /// Returns the outbound stream restart timeout.
  #[must_use]
  pub const fn outbound_restart_timeout(&self) -> Duration {
    self.outbound_restart_timeout
  }

  /// Returns the maximum outbound stream restart count.
  #[must_use]
  pub const fn outbound_max_restarts(&self) -> u32 {
    self.outbound_max_restarts
  }
}
