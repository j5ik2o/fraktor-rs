use super::{Demand, StreamError};

#[cfg(test)]
mod tests;

/// Tracks aggregated demand and handles saturation.
pub struct DemandTracker {
  demand: Demand,
}

impl DemandTracker {
  /// Creates a new tracker with zero demand.
  #[must_use]
  pub const fn new() -> Self {
    Self { demand: Demand::Finite(0) }
  }

  /// Returns the current demand.
  #[must_use]
  pub const fn demand(&self) -> Demand {
    self.demand
  }

  /// Returns `true` if demand is available.
  #[must_use]
  pub const fn has_demand(&self) -> bool {
    self.demand.has_demand()
  }

  /// Adds demand to the tracker.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidDemand`] when `amount` is zero.
  pub const fn request(&mut self, amount: u64) -> Result<(), StreamError> {
    if amount == 0 {
      return Err(StreamError::InvalidDemand { requested: amount });
    }

    match self.demand {
      | Demand::Unbounded => Ok(()),
      | Demand::Finite(current) => {
        let next = current.saturating_add(amount);
        if next == u64::MAX {
          self.demand = Demand::Unbounded;
        } else {
          self.demand = Demand::Finite(next);
        }
        Ok(())
      },
    }
  }

  /// Consumes demand.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::DemandExceeded`] when `amount` exceeds the remaining demand.
  pub const fn consume(&mut self, amount: u64) -> Result<(), StreamError> {
    match self.demand {
      | Demand::Unbounded => Ok(()),
      | Demand::Finite(current) => {
        if amount > current {
          return Err(StreamError::DemandExceeded { requested: amount, remaining: current });
        }
        self.demand = Demand::Finite(current - amount);
        Ok(())
      },
    }
  }
}

impl Default for DemandTracker {
  fn default() -> Self {
    Self::new()
  }
}
