use crate::core::delay_strategy::DelayStrategy;

/// Linear increasing delay strategy.
///
/// Starts at `initial_delay` ticks.  Each time `needs_increase` returns
/// `true` the delay grows by `increase_step` up to `max_delay`.  When
/// `needs_increase` returns `false` the delay resets to `initial_delay`.
pub struct LinearIncreasingDelay<F> {
  increase_step:  u64,
  needs_increase: F,
  initial_delay:  u64,
  max_delay:      u64,
  current_delay:  u64,
}

impl<F> LinearIncreasingDelay<F> {
  /// Creates a new linear increasing delay strategy.
  ///
  /// # Panics
  ///
  /// Panics when `increase_step` is zero or `max_delay <= initial_delay`.
  #[must_use]
  pub fn new(increase_step: u64, needs_increase: F, initial_delay: u64, max_delay: u64) -> Self {
    assert!(increase_step > 0, "increase_step must be positive");
    assert!(max_delay > initial_delay, "max_delay must be greater than initial_delay");
    Self { increase_step, needs_increase, initial_delay, max_delay, current_delay: initial_delay }
  }
}

impl<T, F> DelayStrategy<T> for LinearIncreasingDelay<F>
where
  F: FnMut(&T) -> bool + Send + Sync,
{
  fn next_delay(&mut self, elem: &T) -> u64 {
    if (self.needs_increase)(elem) {
      self.current_delay = self.current_delay.saturating_add(self.increase_step).min(self.max_delay);
    } else {
      self.current_delay = self.initial_delay;
    }
    self.current_delay
  }
}
