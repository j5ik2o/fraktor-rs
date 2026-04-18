#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::any::Any;

use super::{Attribute, MandatoryAttribute};

/// Enables debug-logging behavior for stream stages.
///
/// Mirrors Pekko's `Attributes.DebugLogging(enabled: Boolean)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugLogging(bool);

impl DebugLogging {
  /// Creates a new debug-logging flag.
  #[must_use]
  pub const fn new(enabled: bool) -> Self {
    Self(enabled)
  }

  /// Returns the configured debug-logging flag.
  #[must_use]
  pub const fn value(&self) -> bool {
    self.0
  }
}

impl Attribute for DebugLogging {
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

impl MandatoryAttribute for DebugLogging {}
