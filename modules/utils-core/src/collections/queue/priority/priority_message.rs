use crate::collections::element::Element;

/// Trait for messages with priority.
pub trait PriorityMessage: Element {
  /// Returns the priority of the message, if specified.
  fn get_priority(&self) -> Option<i8>;
}
