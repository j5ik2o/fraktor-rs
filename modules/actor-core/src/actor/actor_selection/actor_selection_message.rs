//! Transport message used to deliver payloads through actor selection paths.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use crate::actor::{actor_selection::SelectionPathElement, messaging::AnyMessage};

/// Message container for actor selection delivery.
#[derive(Debug)]
pub struct ActorSelectionMessage {
  message:          AnyMessage,
  elements:         Vec<SelectionPathElement>,
  wildcard_fan_out: bool,
}

impl ActorSelectionMessage {
  /// Creates a new actor selection transport message.
  #[must_use]
  pub const fn new(message: AnyMessage, elements: Vec<SelectionPathElement>, wildcard_fan_out: bool) -> Self {
    Self { message, elements, wildcard_fan_out }
  }

  /// Returns the nested payload message.
  #[must_use]
  pub const fn message(&self) -> &AnyMessage {
    &self.message
  }

  /// Returns the selection path elements.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // Vec の Deref が const でないため const fn にできない
  pub fn elements(&self) -> &[SelectionPathElement] {
    &self.elements
  }

  /// Returns whether wildcard selection should fan out.
  #[must_use]
  pub const fn wildcard_fan_out(&self) -> bool {
    self.wildcard_fan_out
  }
}
