//! Classic identify message for actor discovery.

#[cfg(test)]
#[path = "identify_test.rs"]
mod tests;

use crate::actor::messaging::{AnyMessage, NotInfluenceReceiveTimeout};

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

impl NotInfluenceReceiveTimeout for Identify {}
