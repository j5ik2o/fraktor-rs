//! Snapshot of at-least-once delivery state.

#[cfg(test)]
#[path = "at_least_once_delivery_snapshot_test.rs"]
mod tests;

use alloc::vec::Vec;

use crate::unconfirmed_delivery::UnconfirmedDelivery;

/// Snapshot of current at-least-once delivery state.
pub struct AtLeastOnceDeliverySnapshot {
  current_delivery_id: u64,
  unconfirmed:         Vec<UnconfirmedDelivery>,
}

impl AtLeastOnceDeliverySnapshot {
  /// Creates a new snapshot.
  #[must_use]
  pub const fn new(current_delivery_id: u64, unconfirmed: Vec<UnconfirmedDelivery>) -> Self {
    Self { current_delivery_id, unconfirmed }
  }

  /// Returns the current delivery id.
  #[must_use]
  pub const fn current_delivery_id(&self) -> u64 {
    self.current_delivery_id
  }

  /// Returns the unconfirmed deliveries.
  #[must_use]
  pub fn unconfirmed_deliveries(&self) -> &[UnconfirmedDelivery] {
    &self.unconfirmed
  }

  /// Consumes the snapshot and returns the unconfirmed deliveries.
  #[must_use]
  pub fn into_unconfirmed(self) -> Vec<UnconfirmedDelivery> {
    self.unconfirmed
  }
}

impl Clone for AtLeastOnceDeliverySnapshot {
  fn clone(&self) -> Self {
    Self { current_delivery_id: self.current_delivery_id, unconfirmed: self.unconfirmed.clone() }
  }
}
