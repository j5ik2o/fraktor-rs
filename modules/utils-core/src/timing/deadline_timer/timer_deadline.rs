use core::time::Duration;

/// A newtype representing a DeadlineTimer deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimerDeadline(Duration);

impl TimerDeadline {
  /// Creates a deadline from the specified duration.
  #[must_use]
  #[inline]
  pub const fn from_duration(duration: Duration) -> Self {
    Self(duration)
  }

  /// Retrieves the stored duration.
  #[must_use]
  #[inline]
  pub const fn as_duration(self) -> Duration {
    self.0
  }
}

impl From<Duration> for TimerDeadline {
  #[inline]
  fn from(value: Duration) -> Self {
    Self::from_duration(value)
  }
}

impl From<TimerDeadline> for Duration {
  #[inline]
  fn from(value: TimerDeadline) -> Self {
    value.as_duration()
  }
}
