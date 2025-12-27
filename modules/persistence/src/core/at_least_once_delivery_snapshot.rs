//! Snapshot representation for at-least-once delivery state.

use alloc::vec::Vec;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::unconfirmed_delivery::UnconfirmedDelivery;

/// Snapshot of at-least-once delivery state.
#[derive(Clone, Debug)]
pub struct AtLeastOnceDeliverySnapshot<TB: RuntimeToolbox + 'static> {
  current_delivery_id: u64,
  unconfirmed:         Vec<UnconfirmedDelivery<TB>>,
}

impl<TB: RuntimeToolbox + 'static> AtLeastOnceDeliverySnapshot<TB> {
  /// Creates a new snapshot instance.
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
  pub fn unconfirmed(&self) -> &[UnconfirmedDelivery<TB>] {
    &self.unconfirmed
  }

  pub(crate) fn into_parts(self) -> (u64, Vec<UnconfirmedDelivery<TB>>) {
    (self.current_delivery_id, self.unconfirmed)
  }
}
