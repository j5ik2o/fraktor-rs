//! Warning payload for unconfirmed at-least-once deliveries.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::unconfirmed_delivery::UnconfirmedDelivery;

/// Warning emitted when unconfirmed deliveries remain.
#[derive(Clone)]
pub struct UnconfirmedWarning<TB: RuntimeToolbox + 'static> {
  unconfirmed: Vec<UnconfirmedDelivery<TB>>,
}

impl<TB: RuntimeToolbox + 'static> UnconfirmedWarning<TB> {
  /// Creates a warning from unconfirmed deliveries.
  #[must_use]
  pub const fn new(unconfirmed: Vec<UnconfirmedDelivery<TB>>) -> Self {
    Self { unconfirmed }
  }

  /// Returns tracked unconfirmed deliveries.
  #[must_use]
  pub fn unconfirmed_deliveries(&self) -> &[UnconfirmedDelivery<TB>] {
    &self.unconfirmed
  }

  /// Returns the number of unconfirmed deliveries.
  #[must_use]
  pub const fn count(&self) -> usize {
    self.unconfirmed.len()
  }
}
