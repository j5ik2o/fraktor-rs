//! Warning payload for unconfirmed at-least-once deliveries.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use crate::unconfirmed_delivery::UnconfirmedDelivery;

/// Warning emitted when unconfirmed deliveries remain.
#[derive(Clone)]
pub struct UnconfirmedWarning {
  unconfirmed: Vec<UnconfirmedDelivery>,
}

impl UnconfirmedWarning {
  /// Creates a warning from unconfirmed deliveries.
  #[must_use]
  pub const fn new(unconfirmed: Vec<UnconfirmedDelivery>) -> Self {
    Self { unconfirmed }
  }

  /// Returns tracked unconfirmed deliveries.
  #[must_use]
  pub fn unconfirmed_deliveries(&self) -> &[UnconfirmedDelivery] {
    &self.unconfirmed
  }

  /// Returns the number of unconfirmed deliveries.
  #[must_use]
  pub const fn count(&self) -> usize {
    self.unconfirmed.len()
  }
}
