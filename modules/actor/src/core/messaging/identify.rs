//! Classic identify message for actor discovery.

#[cfg(test)]
mod tests;

use crate::core::messaging::AnyMessage;

/// Message requesting the target actor to reply with its identity.
#[derive(Clone, Debug)]
pub struct Identify {
  correlation_id: AnyMessage,
}

impl Identify {
  /// Creates a new identify message.
  #[must_use]
  pub const fn new(correlation_id: AnyMessage) -> Self {
    Self { correlation_id }
  }

  /// Returns the correlation identifier carried by this request.
  #[must_use]
  pub const fn correlation_id(&self) -> &AnyMessage {
    &self.correlation_id
  }
}
