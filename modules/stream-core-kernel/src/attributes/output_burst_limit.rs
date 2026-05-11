#[cfg(test)]
#[path = "output_burst_limit_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::any::Any;

use super::{Attribute, MandatoryAttribute};

/// Maximum burst size emitted before yielding to other stages.
///
/// Mirrors Pekko's `Attributes.OutputBurstLimit(limit: Int)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputBurstLimit(usize);

impl OutputBurstLimit {
  /// Creates a new output burst limit.
  #[must_use]
  pub const fn new(limit: usize) -> Self {
    Self(limit)
  }

  /// Returns the configured burst limit.
  #[must_use]
  pub const fn value(&self) -> usize {
    self.0
  }
}

impl Attribute for OutputBurstLimit {
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

impl MandatoryAttribute for OutputBurstLimit {}
