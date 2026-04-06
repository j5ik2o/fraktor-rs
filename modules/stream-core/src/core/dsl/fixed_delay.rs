use crate::core::dsl::DelayStrategy;

/// Fixed delay strategy that always returns a constant delay.
pub struct FixedDelay {
  delay_ticks: u64,
}

impl FixedDelay {
  /// Creates a fixed delay strategy with the given tick count.
  #[must_use]
  pub const fn new(delay_ticks: u64) -> Self {
    Self { delay_ticks }
  }
}

impl<T> DelayStrategy<T> for FixedDelay {
  fn next_delay(&mut self, _elem: &T) -> u64 {
    self.delay_ticks
  }
}
