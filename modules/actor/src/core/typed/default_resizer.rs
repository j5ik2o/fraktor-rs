//! Default threshold-based pool router resizer.

use super::resizer::Resizer;

#[cfg(test)]
mod tests;

/// A simple threshold-based resizer that keeps the pool within bounds.
///
/// Checks for resizing every `messages_per_resize` messages and adjusts
/// the pool size to stay within `[lower_bound, upper_bound]`.
pub struct DefaultResizer {
  lower_bound:         usize,
  upper_bound:         usize,
  messages_per_resize: u64,
}

impl DefaultResizer {
  /// Creates a new default resizer.
  ///
  /// # Panics
  ///
  /// Panics if `lower_bound` is zero, `upper_bound < lower_bound`,
  /// or `messages_per_resize` is zero.
  #[must_use]
  pub fn new(lower_bound: usize, upper_bound: usize, messages_per_resize: u64) -> Self {
    assert!(lower_bound > 0, "lower_bound must be positive");
    assert!(upper_bound >= lower_bound, "upper_bound must be >= lower_bound");
    assert!(messages_per_resize > 0, "messages_per_resize must be positive");
    Self { lower_bound, upper_bound, messages_per_resize }
  }

  /// Returns the configured lower bound.
  #[must_use]
  pub const fn lower_bound(&self) -> usize {
    self.lower_bound
  }

  /// Returns the configured upper bound.
  #[must_use]
  pub const fn upper_bound(&self) -> usize {
    self.upper_bound
  }

  /// Returns the configured messages per resize check.
  #[must_use]
  pub const fn messages_per_resize(&self) -> u64 {
    self.messages_per_resize
  }
}

impl Resizer for DefaultResizer {
  fn is_time_for_resize(&self, message_counter: u64) -> bool {
    message_counter.is_multiple_of(self.messages_per_resize)
  }

  fn resize(&self, current_routee_count: usize) -> i32 {
    if current_routee_count < self.lower_bound {
      (self.lower_bound - current_routee_count) as i32
    } else if current_routee_count > self.upper_bound {
      -((current_routee_count - self.upper_bound) as i32)
    } else {
      0
    }
  }
}
