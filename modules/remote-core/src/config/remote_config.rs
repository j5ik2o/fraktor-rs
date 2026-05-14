//! Typed `RemoteConfig` with a `self`-consuming builder API.

use alloc::string::String;
use core::time::Duration;

use crate::config::{LargeMessageDestinations, RemoteCompressionConfig};

/// Default handshake timeout (20 seconds), matching Pekko Artery advanced defaults.
pub(crate) const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(20);

/// Default shutdown flush timeout (5 seconds).
const DEFAULT_SHUTDOWN_FLUSH_TIMEOUT: Duration = Duration::from_secs(5);

/// Default flight recorder ring buffer capacity.
const DEFAULT_FLIGHT_RECORDER_CAPACITY: usize = 1024;

/// Default ack-based redelivery send window (Pekko Artery default).
pub(crate) const DEFAULT_ACK_SEND_WINDOW: u32 = 1024;

/// Default ack-based redelivery receive window (Pekko Artery default).
pub(crate) const DEFAULT_ACK_RECEIVE_WINDOW: u32 = 1024;

/// Default system message buffer size.
const DEFAULT_SYSTEM_MESSAGE_BUFFER_SIZE: usize = 20_000;

/// Default outbound message queue size.
pub(crate) const DEFAULT_OUTBOUND_MESSAGE_QUEUE_SIZE: usize = 3072;

/// Default outbound control queue size.
pub(crate) const DEFAULT_OUTBOUND_CONTROL_QUEUE_SIZE: usize = 20_000;

/// Default outbound large-message queue size.
pub(crate) const DEFAULT_OUTBOUND_LARGE_MESSAGE_QUEUE_SIZE: usize = 256;

/// Default remote event queue size.
///
/// The core event queue absorbs both outbound message and outbound control
/// producers, so the default capacity is sized to their combined queues.
const DEFAULT_REMOTE_EVENT_QUEUE_SIZE: usize =
  DEFAULT_OUTBOUND_MESSAGE_QUEUE_SIZE + DEFAULT_OUTBOUND_CONTROL_QUEUE_SIZE;

/// Default outbound high watermark.
const DEFAULT_OUTBOUND_HIGH_WATERMARK: usize = 1024;

/// Default outbound low watermark.
const DEFAULT_OUTBOUND_LOW_WATERMARK: usize = 512;

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

/// Default duration before removing unused quarantined associations.
pub(crate) const DEFAULT_REMOVE_QUARANTINED_ASSOCIATION_AFTER: Duration = Duration::from_secs(60 * 60);

/// Default outbound stream restart backoff.
const DEFAULT_OUTBOUND_RESTART_BACKOFF: Duration = Duration::from_secs(1);

/// Default outbound stream restart timeout.
const DEFAULT_OUTBOUND_RESTART_TIMEOUT: Duration = Duration::from_secs(5);

/// Default maximum outbound stream restart count.
const DEFAULT_OUTBOUND_MAX_RESTARTS: u32 = 5;

/// Default inbound lane count.
const DEFAULT_INBOUND_LANES: usize = 4;

/// Default outbound lane count.
const DEFAULT_OUTBOUND_LANES: usize = 1;

/// Default maximum wire frame size.
const DEFAULT_MAXIMUM_FRAME_SIZE: usize = 256 * 1024;

/// Default direct buffer pool size.
const DEFAULT_BUFFER_POOL_SIZE: usize = 128;

/// Minimum accepted maximum wire frame size.
const MINIMUM_MAXIMUM_FRAME_SIZE: usize = 32 * 1024;

/// Typed remote subsystem configuration.
///
/// Modeled after Pekko Artery's `RemoteSettings` (`RemoteConfig` in fraktor-rs), expressed as a
/// pure Rust struct with a `self`-consuming builder API (see Decision 11). The
/// ack-based redelivery and Artery advanced timing fields provide the core contract used by the
/// `std` adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteConfig {
  canonical_host: String,
  canonical_port: Option<u16>,
  bind_hostname: Option<String>,
  bind_port: Option<u16>,
  handshake_timeout: Duration,
  shutdown_flush_timeout: Duration,
  flight_recorder_capacity: usize,
  ack_send_window: u32,
  ack_receive_window: u32,
  system_message_buffer_size: usize,
  outbound_message_queue_size: usize,
  outbound_control_queue_size: usize,
  outbound_large_message_queue_size: usize,
  remote_event_queue_size: usize,
  outbound_high_watermark: usize,
  outbound_low_watermark: usize,
  large_message_destinations: LargeMessageDestinations,
  system_message_resend_interval: Duration,
  give_up_system_message_after: Duration,
  handshake_retry_interval: Duration,
  inject_handshake_interval: Duration,
  stop_idle_outbound_after: Duration,
  quarantine_idle_outbound_after: Duration,
  stop_quarantined_after_idle: Duration,
  remove_quarantined_association_after: Duration,
  outbound_restart_backoff: Duration,
  outbound_restart_timeout: Duration,
  outbound_max_restarts: u32,
  compression_config: RemoteCompressionConfig,
  inbound_lanes: usize,
  outbound_lanes: usize,
  maximum_frame_size: usize,
  buffer_pool_size: usize,
  untrusted_mode: bool,
  log_received_messages: bool,
  log_sent_messages: bool,
  log_frame_size_exceeding: Option<usize>,
}

impl RemoteConfig {
  /// Creates a new [`RemoteConfig`] with the given canonical host and default values
  /// for every other field.
  #[must_use]
  pub fn new(canonical_host: impl Into<String>) -> Self {
    Self {
      canonical_host: canonical_host.into(),
      canonical_port: None,
      bind_hostname: None,
      bind_port: None,
      handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
      shutdown_flush_timeout: DEFAULT_SHUTDOWN_FLUSH_TIMEOUT,
      flight_recorder_capacity: DEFAULT_FLIGHT_RECORDER_CAPACITY,
      ack_send_window: DEFAULT_ACK_SEND_WINDOW,
      ack_receive_window: DEFAULT_ACK_RECEIVE_WINDOW,
      system_message_buffer_size: DEFAULT_SYSTEM_MESSAGE_BUFFER_SIZE,
      outbound_message_queue_size: DEFAULT_OUTBOUND_MESSAGE_QUEUE_SIZE,
      outbound_control_queue_size: DEFAULT_OUTBOUND_CONTROL_QUEUE_SIZE,
      outbound_large_message_queue_size: DEFAULT_OUTBOUND_LARGE_MESSAGE_QUEUE_SIZE,
      remote_event_queue_size: DEFAULT_REMOTE_EVENT_QUEUE_SIZE,
      outbound_high_watermark: DEFAULT_OUTBOUND_HIGH_WATERMARK,
      outbound_low_watermark: DEFAULT_OUTBOUND_LOW_WATERMARK,
      large_message_destinations: LargeMessageDestinations::new(),
      system_message_resend_interval: DEFAULT_SYSTEM_MESSAGE_RESEND_INTERVAL,
      give_up_system_message_after: DEFAULT_GIVE_UP_SYSTEM_MESSAGE_AFTER,
      handshake_retry_interval: DEFAULT_HANDSHAKE_RETRY_INTERVAL,
      inject_handshake_interval: DEFAULT_INJECT_HANDSHAKE_INTERVAL,
      stop_idle_outbound_after: DEFAULT_STOP_IDLE_OUTBOUND_AFTER,
      quarantine_idle_outbound_after: DEFAULT_QUARANTINE_IDLE_OUTBOUND_AFTER,
      stop_quarantined_after_idle: DEFAULT_STOP_QUARANTINED_AFTER_IDLE,
      remove_quarantined_association_after: DEFAULT_REMOVE_QUARANTINED_ASSOCIATION_AFTER,
      outbound_restart_backoff: DEFAULT_OUTBOUND_RESTART_BACKOFF,
      outbound_restart_timeout: DEFAULT_OUTBOUND_RESTART_TIMEOUT,
      outbound_max_restarts: DEFAULT_OUTBOUND_MAX_RESTARTS,
      compression_config: RemoteCompressionConfig::new(),
      inbound_lanes: DEFAULT_INBOUND_LANES,
      outbound_lanes: DEFAULT_OUTBOUND_LANES,
      maximum_frame_size: DEFAULT_MAXIMUM_FRAME_SIZE,
      buffer_pool_size: DEFAULT_BUFFER_POOL_SIZE,
      untrusted_mode: false,
      log_received_messages: false,
      log_sent_messages: false,
      log_frame_size_exceeding: None,
    }
  }

  /// Returns a copy with the given canonical port.
  #[must_use]
  pub const fn with_canonical_port(mut self, port: u16) -> Self {
    self.canonical_port = Some(port);
    self
  }

  /// Returns a copy with the given bind host name.
  #[must_use]
  pub fn with_bind_hostname(mut self, hostname: impl Into<String>) -> Self {
    self.bind_hostname = Some(hostname.into());
    self
  }

  /// Returns a copy with the given bind port.
  #[must_use]
  pub const fn with_bind_port(mut self, port: u16) -> Self {
    self.bind_port = Some(port);
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

  /// Returns a copy with the given outbound message queue size.
  ///
  /// # Panics
  ///
  /// Panics when `size` is zero.
  #[must_use]
  pub const fn with_outbound_message_queue_size(mut self, size: usize) -> Self {
    assert!(size > 0, "outbound message queue size must be greater than zero");
    self.outbound_message_queue_size = size;
    self
  }

  /// Returns a copy with the given outbound control queue size.
  ///
  /// # Panics
  ///
  /// Panics when `size` is zero.
  #[must_use]
  pub const fn with_outbound_control_queue_size(mut self, size: usize) -> Self {
    assert!(size > 0, "outbound control queue size must be greater than zero");
    self.outbound_control_queue_size = size;
    self
  }

  /// Returns a copy with the given outbound large-message queue size.
  ///
  /// # Panics
  ///
  /// Panics when `size` is zero.
  #[must_use]
  pub const fn with_outbound_large_message_queue_size(mut self, size: usize) -> Self {
    assert!(size > 0, "outbound large-message queue size must be greater than zero");
    self.outbound_large_message_queue_size = size;
    self
  }

  /// Returns a copy with the given remote event queue size.
  ///
  /// # Panics
  ///
  /// Panics when `size` is zero.
  #[must_use]
  pub const fn with_remote_event_queue_size(mut self, size: usize) -> Self {
    assert!(size > 0, "remote event queue size must be greater than zero");
    self.remote_event_queue_size = size;
    self
  }

  /// Returns a copy with the given outbound high watermark.
  ///
  /// If the current low watermark is no longer lower than `high`, the low
  /// watermark is lowered to keep the pair valid. Use
  /// [`Self::with_outbound_watermarks`] when changing both values explicitly.
  ///
  /// # Panics
  ///
  /// Panics when `high < 2`. The auto-adjusted low watermark would otherwise
  /// drop to zero, making the release condition `queue_len < low` unreachable.
  #[must_use]
  pub const fn with_outbound_high_watermark(mut self, high: usize) -> Self {
    assert!(high >= 2, "outbound high watermark must be at least 2 to keep the auto-adjusted low watermark reachable");
    self.outbound_high_watermark = high;
    if self.outbound_low_watermark >= high {
      self.outbound_low_watermark = high - 1;
    }
    self
  }

  /// Returns a copy with the given outbound low watermark.
  ///
  /// If the current high watermark is no longer higher than `low`, the high
  /// watermark is raised to keep the pair valid. Use
  /// [`Self::with_outbound_watermarks`] when changing both values explicitly.
  ///
  /// # Panics
  ///
  /// Panics when `low` is `0` (the release condition `queue_len < low` would
  /// be unreachable for any non-empty queue) or `low == usize::MAX` (no
  /// representable high watermark above it).
  #[must_use]
  pub const fn with_outbound_low_watermark(mut self, low: usize) -> Self {
    assert!(low > 0, "outbound low watermark must be greater than zero so the release condition can fire");
    assert!(low < usize::MAX, "outbound low watermark must be lower than usize::MAX");
    self.outbound_low_watermark = low;
    if self.outbound_high_watermark <= low {
      self.outbound_high_watermark = low + 1;
    }
    self
  }

  /// Returns a copy with both outbound watermarks set atomically.
  ///
  /// # Panics
  ///
  /// Panics when `low == 0` (release condition unreachable) or
  /// `low >= high` (invalid pair).
  #[must_use]
  pub const fn with_outbound_watermarks(mut self, low: usize, high: usize) -> Self {
    assert!(low > 0, "outbound low watermark must be greater than zero so the release condition can fire");
    assert!(low < high, "outbound low watermark must be lower than high watermark");
    self.outbound_low_watermark = low;
    self.outbound_high_watermark = high;
    self
  }

  /// Returns a copy with the configured large-message destination patterns.
  #[must_use]
  pub fn with_large_message_destinations(mut self, destinations: LargeMessageDestinations) -> Self {
    self.large_message_destinations = destinations;
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

  /// Returns a copy with the duration before removing unused quarantined associations.
  ///
  /// # Panics
  ///
  /// Panics when `duration` is zero.
  #[must_use]
  pub const fn with_remove_quarantined_association_after(mut self, duration: Duration) -> Self {
    assert!(!duration.is_zero(), "remove quarantined association after must be greater than zero");
    self.remove_quarantined_association_after = duration;
    self
  }

  /// Returns a copy with the given outbound stream restart backoff.
  ///
  /// # Panics
  ///
  /// Panics when `duration` is zero.
  #[must_use]
  pub const fn with_outbound_restart_backoff(mut self, duration: Duration) -> Self {
    assert!(!duration.is_zero(), "outbound restart backoff must be greater than zero");
    self.outbound_restart_backoff = duration;
    self
  }

  /// Returns a copy with the given outbound stream restart timeout.
  ///
  /// # Panics
  ///
  /// Panics when `duration` is zero.
  #[must_use]
  pub const fn with_outbound_restart_timeout(mut self, duration: Duration) -> Self {
    assert!(!duration.is_zero(), "outbound restart timeout must be greater than zero");
    self.outbound_restart_timeout = duration;
    self
  }

  /// Returns a copy with the given maximum outbound stream restart count.
  #[must_use]
  pub const fn with_outbound_max_restarts(mut self, max_restarts: u32) -> Self {
    self.outbound_max_restarts = max_restarts;
    self
  }

  /// Returns a copy with the given compression settings.
  #[must_use]
  pub const fn with_compression_config(mut self, compression_config: RemoteCompressionConfig) -> Self {
    self.compression_config = compression_config;
    self
  }

  /// Returns a copy with the given inbound lane count.
  ///
  /// # Panics
  ///
  /// Panics when `lanes` is zero.
  #[must_use]
  pub const fn with_inbound_lanes(mut self, lanes: usize) -> Self {
    assert!(lanes > 0, "inbound lanes must be greater than zero");
    self.inbound_lanes = lanes;
    self
  }

  /// Returns a copy with the given outbound lane count.
  ///
  /// # Panics
  ///
  /// Panics when `lanes` is zero.
  #[must_use]
  pub const fn with_outbound_lanes(mut self, lanes: usize) -> Self {
    assert!(lanes > 0, "outbound lanes must be greater than zero");
    self.outbound_lanes = lanes;
    self
  }

  /// Returns a copy with the given maximum wire frame size.
  ///
  /// # Panics
  ///
  /// Panics when `size` is smaller than 32 KiB.
  #[must_use]
  pub const fn with_maximum_frame_size(mut self, size: usize) -> Self {
    assert!(size >= MINIMUM_MAXIMUM_FRAME_SIZE, "maximum frame size must be at least 32 KiB");
    self.maximum_frame_size = size;
    self
  }

  /// Returns a copy with the given direct buffer pool size.
  ///
  /// # Panics
  ///
  /// Panics when `size` is zero.
  #[must_use]
  pub const fn with_buffer_pool_size(mut self, size: usize) -> Self {
    assert!(size > 0, "buffer pool size must be greater than zero");
    self.buffer_pool_size = size;
    self
  }

  /// Returns a copy with untrusted mode enabled or disabled.
  #[must_use]
  pub const fn with_untrusted_mode(mut self, enabled: bool) -> Self {
    self.untrusted_mode = enabled;
    self
  }

  /// Returns a copy with received-message logging enabled or disabled.
  #[must_use]
  pub const fn with_log_received_messages(mut self, enabled: bool) -> Self {
    self.log_received_messages = enabled;
    self
  }

  /// Returns a copy with sent-message logging enabled or disabled.
  #[must_use]
  pub const fn with_log_sent_messages(mut self, enabled: bool) -> Self {
    self.log_sent_messages = enabled;
    self
  }

  /// Returns a copy with a frame-size logging threshold.
  #[must_use]
  pub const fn with_log_frame_size_exceeding(mut self, threshold: usize) -> Self {
    self.log_frame_size_exceeding = Some(threshold);
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

  /// Returns the bind host name, if configured.
  #[must_use]
  pub fn bind_hostname(&self) -> Option<&str> {
    self.bind_hostname.as_deref()
  }

  /// Returns the bind port, if configured.
  #[must_use]
  pub const fn bind_port(&self) -> Option<u16> {
    self.bind_port
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

  /// Returns the outbound message queue size.
  #[must_use]
  pub const fn outbound_message_queue_size(&self) -> usize {
    self.outbound_message_queue_size
  }

  /// Returns the outbound control queue size.
  #[must_use]
  pub const fn outbound_control_queue_size(&self) -> usize {
    self.outbound_control_queue_size
  }

  /// Returns the outbound large-message queue size.
  #[must_use]
  pub const fn outbound_large_message_queue_size(&self) -> usize {
    self.outbound_large_message_queue_size
  }

  /// Returns the remote event queue size.
  #[must_use]
  pub const fn remote_event_queue_size(&self) -> usize {
    self.remote_event_queue_size
  }

  /// Returns the outbound high watermark.
  #[must_use]
  pub const fn outbound_high_watermark(&self) -> usize {
    self.outbound_high_watermark
  }

  /// Returns the outbound low watermark.
  #[must_use]
  pub const fn outbound_low_watermark(&self) -> usize {
    self.outbound_low_watermark
  }

  /// Returns the configured large-message destination patterns.
  #[must_use]
  pub const fn large_message_destinations(&self) -> &LargeMessageDestinations {
    &self.large_message_destinations
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

  /// Returns the duration before removing unused quarantined associations.
  #[must_use]
  pub const fn remove_quarantined_association_after(&self) -> Duration {
    self.remove_quarantined_association_after
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

  /// Returns the compression settings surface.
  #[must_use]
  pub const fn compression_config(&self) -> &RemoteCompressionConfig {
    &self.compression_config
  }

  /// Returns the inbound lane count.
  #[must_use]
  pub const fn inbound_lanes(&self) -> usize {
    self.inbound_lanes
  }

  /// Returns the outbound lane count.
  #[must_use]
  pub const fn outbound_lanes(&self) -> usize {
    self.outbound_lanes
  }

  /// Returns the maximum wire frame size.
  #[must_use]
  pub const fn maximum_frame_size(&self) -> usize {
    self.maximum_frame_size
  }

  /// Returns the direct buffer pool size.
  #[must_use]
  pub const fn buffer_pool_size(&self) -> usize {
    self.buffer_pool_size
  }

  /// Returns whether untrusted mode is enabled.
  #[must_use]
  pub const fn untrusted_mode(&self) -> bool {
    self.untrusted_mode
  }

  /// Returns whether received-message logging is enabled.
  #[must_use]
  pub const fn log_received_messages(&self) -> bool {
    self.log_received_messages
  }

  /// Returns whether sent-message logging is enabled.
  #[must_use]
  pub const fn log_sent_messages(&self) -> bool {
    self.log_sent_messages
  }

  /// Returns the frame-size logging threshold, if configured.
  #[must_use]
  pub const fn log_frame_size_exceeding(&self) -> Option<usize> {
    self.log_frame_size_exceeding
  }
}
