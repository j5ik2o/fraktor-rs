//! Snapshot of at-least-once delivery state.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::unconfirmed_delivery::UnconfirmedDelivery;

/// Snapshot of current at-least-once delivery state.
pub struct AtLeastOnceDeliverySnapshot<TB: RuntimeToolbox + 'static> {
  current_delivery_id: u64,
  unconfirmed:         Vec<UnconfirmedDelivery<TB>>,
}

impl<TB: RuntimeToolbox + 'static> AtLeastOnceDeliverySnapshot<TB> {
  /// Creates a new snapshot.
  #[must_use]
  pub const fn new(current_delivery_id: u64, unconfirmed: Vec<UnconfirmedDelivery<TB>>) -> Self {
    Self { current_delivery_id, unconfirmed }
  }

  /// Returns the current delivery id.
  #[must_use]
  pub const fn current_delivery_id(&self) -> u64 {
    self.current_delivery_id
  }

  /// Returns the unconfirmed deliveries.
  #[must_use]
  pub fn unconfirmed_deliveries(&self) -> &[UnconfirmedDelivery<TB>] {
    &self.unconfirmed
  }

  /// Consumes the snapshot and returns the unconfirmed deliveries.
  #[must_use]
  pub fn into_unconfirmed(self) -> Vec<UnconfirmedDelivery<TB>> {
    self.unconfirmed
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for AtLeastOnceDeliverySnapshot<TB> {
  fn clone(&self) -> Self {
    Self { current_delivery_id: self.current_delivery_id, unconfirmed: self.unconfirmed.clone() }
  }
}
