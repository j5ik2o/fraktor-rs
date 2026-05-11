#[cfg(test)]
#[path = "sync_processing_limit_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::any::Any;

use super::{Attribute, MandatoryAttribute};

/// Maximum number of elements processed synchronously before yielding.
///
/// Mirrors Pekko's `Attributes.SyncProcessingLimit(limit: Int)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncProcessingLimit(usize);

impl SyncProcessingLimit {
  /// Creates a new sync processing limit.
  #[must_use]
  pub const fn new(limit: usize) -> Self {
    Self(limit)
  }

  /// Returns the configured sync processing limit.
  #[must_use]
  pub const fn value(&self) -> usize {
    self.0
  }
}

impl Attribute for SyncProcessingLimit {
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

impl MandatoryAttribute for SyncProcessingLimit {}
