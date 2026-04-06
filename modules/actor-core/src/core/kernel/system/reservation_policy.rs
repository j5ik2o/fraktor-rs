//! UID reservation policy configuration.

use core::time::Duration;

/// Default UID reservation period (5 days).
pub(crate) const DEFAULT_QUARANTINE_DURATION: Duration = Duration::from_secs(5 * 24 * 3600);

/// Policy for UID reservations and quarantine duration.
#[derive(Clone, Debug)]
pub struct ReservationPolicy {
  quarantine_duration: Duration,
}

impl ReservationPolicy {
  /// Creates a policy with custom quarantine duration.
  #[must_use]
  pub const fn with_quarantine_duration(duration: Duration) -> Self {
    Self { quarantine_duration: duration }
  }

  /// Returns the configured quarantine duration.
  #[must_use]
  pub const fn quarantine_duration(&self) -> Duration {
    self.quarantine_duration
  }
}

impl Default for ReservationPolicy {
  fn default() -> Self {
    Self { quarantine_duration: DEFAULT_QUARANTINE_DURATION }
  }
}
