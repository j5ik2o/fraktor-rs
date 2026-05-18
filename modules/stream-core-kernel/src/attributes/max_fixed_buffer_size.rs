#[cfg(test)]
#[path = "max_fixed_buffer_size_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::any::Any;

use super::{Attribute, MandatoryAttribute};

/// Maximum fixed-size buffer capacity for stream stages.
///
/// Mirrors Pekko's `Attributes.MaxFixedBufferSize(size: Int)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaxFixedBufferSize(usize);

impl MaxFixedBufferSize {
  /// Creates a new max fixed buffer size attribute.
  #[must_use]
  pub const fn new(size: usize) -> Self {
    Self(size)
  }

  /// Returns the configured maximum buffer size.
  #[must_use]
  pub const fn value(&self) -> usize {
    self.0
  }
}

impl Attribute for MaxFixedBufferSize {
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

impl MandatoryAttribute for MaxFixedBufferSize {}
