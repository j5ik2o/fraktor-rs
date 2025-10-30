use core::fmt::Debug;

use crate::SharedBound;

/// Trait for messages with priority.
pub trait PriorityMessage: Debug + SharedBound + 'static {
  /// Returns the priority of the message, if specified.
  fn get_priority(&self) -> Option<i8>;
}
