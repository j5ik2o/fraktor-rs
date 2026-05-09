#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::any::Any;

use super::{Attribute, MandatoryAttribute};

/// Configures the input buffer size for a stream stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputBuffer {
  /// Initial buffer capacity.
  pub initial: usize,
  /// Maximum buffer capacity.
  pub max:     usize,
}

impl InputBuffer {
  /// Creates a new input buffer configuration.
  #[must_use]
  pub const fn new(initial: usize, max: usize) -> Self {
    Self { initial, max }
  }
}

impl Attribute for InputBuffer {
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn clone_box(&self) -> Box<dyn Attribute> {
    Box::new(self.clone())
  }

  fn eq_attr(&self, other: &dyn Any) -> bool {
    other.downcast_ref::<Self>() == Some(self)
  }
}

impl MandatoryAttribute for InputBuffer {}
