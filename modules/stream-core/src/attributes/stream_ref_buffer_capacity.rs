#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::any::Any;

use super::Attribute;
use crate::{StreamDslError, validate_positive_argument};

const CAPACITY_ARGUMENT_NAME: &str = "capacity";

/// Receiver-side eager buffer capacity for stream references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamRefBufferCapacity {
  /// Receiver-side eager buffer capacity.
  pub capacity: usize,
}

impl StreamRefBufferCapacity {
  /// Creates a stream reference buffer capacity attribute.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError::InvalidArgument`] when `capacity == 0`.
  pub const fn new(capacity: usize) -> Result<Self, StreamDslError> {
    match validate_positive_argument(CAPACITY_ARGUMENT_NAME, capacity) {
      | Ok(capacity) => Ok(Self { capacity }),
      | Err(error) => Err(error),
    }
  }
}

impl Attribute for StreamRefBufferCapacity {
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn clone_box(&self) -> Box<dyn Attribute> {
    Box::new(*self)
  }

  fn eq_attr(&self, other: &dyn Any) -> bool {
    other.downcast_ref::<Self>() == Some(self)
  }
}
