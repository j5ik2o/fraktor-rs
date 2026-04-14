//! Configuration for the consumer controller.

use core::time::Duration;

#[cfg(test)]
mod tests;

/// Default flow-control window size.
const DEFAULT_FLOW_CONTROL_WINDOW: u32 = 50;

/// Default minimum resend interval.
const DEFAULT_RESEND_INTERVAL_MIN: Duration = Duration::from_secs(2);

/// Default maximum resend interval.
const DEFAULT_RESEND_INTERVAL_MAX: Duration = Duration::from_secs(30);

/// Settings for [`ConsumerController`](super::ConsumerController).
///
/// Corresponds to Pekko's `ConsumerController.Settings`.
#[derive(Debug, Clone)]
pub struct ConsumerControllerConfig {
  flow_control_window: u32,
  only_flow_control:   bool,
  resend_interval_min: Duration,
  resend_interval_max: Duration,
}

impl ConsumerControllerConfig {
  /// Creates default settings for in-memory reliable delivery.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      flow_control_window: DEFAULT_FLOW_CONTROL_WINDOW,
      only_flow_control:   false,
      resend_interval_min: DEFAULT_RESEND_INTERVAL_MIN,
      resend_interval_max: DEFAULT_RESEND_INTERVAL_MAX,
    }
  }

  /// Returns the flow-control window size.
  ///
  /// This determines how many unconfirmed messages the consumer side will
  /// request from the producer side at a time.
  #[must_use]
  pub const fn flow_control_window(&self) -> u32 {
    self.flow_control_window
  }

  /// Returns a new settings with the given flow-control window.
  #[must_use]
  pub const fn with_flow_control_window(self, window: u32) -> Self {
    let clamped = if window == 0 { 1 } else { window };
    Self { flow_control_window: clamped, ..self }
  }

  /// Returns whether only flow-control is used (no resend of lost messages).
  #[must_use]
  pub const fn only_flow_control(&self) -> bool {
    self.only_flow_control
  }

  /// Returns a new settings with `only_flow_control` set.
  #[must_use]
  pub const fn with_only_flow_control(self, value: bool) -> Self {
    Self { only_flow_control: value, ..self }
  }

  /// Returns the minimum resend interval for unconfirmed messages.
  ///
  /// Corresponds to Pekko's `ConsumerController.Settings.resendIntervalMin`.
  #[must_use]
  pub const fn resend_interval_min(&self) -> Duration {
    self.resend_interval_min
  }

  /// Returns a new settings with the given minimum resend interval.
  ///
  /// Corresponds to Pekko's `ConsumerController.Settings.withResendIntervalMin`.
  #[must_use]
  pub const fn with_resend_interval_min(self, interval: Duration) -> Self {
    Self { resend_interval_min: interval, ..self }
  }

  /// Returns the maximum resend interval for unconfirmed messages.
  ///
  /// Corresponds to Pekko's `ConsumerController.Settings.resendIntervalMax`.
  #[must_use]
  pub const fn resend_interval_max(&self) -> Duration {
    self.resend_interval_max
  }

  /// Returns a new settings with the given maximum resend interval.
  ///
  /// Corresponds to Pekko's `ConsumerController.Settings.withResendIntervalMax`.
  #[must_use]
  pub const fn with_resend_interval_max(self, interval: Duration) -> Self {
    Self { resend_interval_max: interval, ..self }
  }
}

impl Default for ConsumerControllerConfig {
  fn default() -> Self {
    Self::new()
  }
}
