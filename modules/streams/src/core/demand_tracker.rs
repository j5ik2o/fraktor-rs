//! Demand tracker implementation.

#[cfg(test)]
mod tests;

use crate::core::{demand::Demand, stream_error::StreamError};

/// Tracks downstream demand.
#[derive(Debug, Clone)]
pub struct DemandTracker {
  current: Demand,
}

impl DemandTracker {
  /// Creates a new demand tracker with zero demand.
  #[must_use]
  pub const fn new() -> Self {
    Self { current: Demand::Finite(0) }
  }

  /// Returns the default demand tracker.
  #[must_use]
  pub const fn default_value() -> Self {
    Self::new()
  }

  /// Returns the current demand value.
  #[must_use]
  pub const fn current(&self) -> Demand {
    self.current
  }

  /// Adds demand to the tracker.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::InvalidDemand` when `amount` is zero.
  pub const fn request(&mut self, amount: u64) -> Result<Demand, StreamError> {
    if amount == 0 {
      return Err(StreamError::InvalidDemand);
    }

    self.current = match self.current {
      | Demand::Unbounded => Demand::Unbounded,
      | Demand::Finite(current) => match current.checked_add(amount) {
        | Some(total) => Demand::Finite(total),
        | None => Demand::Unbounded,
      },
    };
    Ok(self.current)
  }

  /// Consumes a single unit of demand when available.
  #[must_use]
  pub const fn consume_one(&mut self) -> bool {
    match self.current {
      | Demand::Unbounded => true,
      | Demand::Finite(value) if value > 0 => {
        self.current = Demand::Finite(value - 1);
        true
      },
      | Demand::Finite(_) => false,
    }
  }
}

impl Default for DemandTracker {
  fn default() -> Self {
    Self::new()
  }
}
