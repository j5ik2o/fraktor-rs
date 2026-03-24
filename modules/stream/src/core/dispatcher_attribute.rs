//! Dispatcher attribute for stream graph island execution.

use alloc::string::String;
use core::any::Any;

use super::Attribute;

/// Specifies which dispatcher an async island should use.
///
/// When present on a graph node, it implies an async boundary and
/// additionally selects the named dispatcher for the island's
/// execution context.  This mirrors Pekko's
/// `ActorAttributes.Dispatcher`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatcherAttribute {
  name: String,
}

impl DispatcherAttribute {
  /// Creates a new dispatcher attribute with the given name.
  #[must_use]
  pub fn new(name: impl Into<String>) -> Self {
    Self { name: name.into() }
  }

  /// Returns the dispatcher name.
  #[must_use]
  pub fn name(&self) -> &str {
    &self.name
  }
}

impl Attribute for DispatcherAttribute {
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn clone_box(&self) -> alloc::boxed::Box<dyn Attribute> {
    alloc::boxed::Box::new(self.clone())
  }

  fn eq_attr(&self, other: &dyn Any) -> bool {
    other.downcast_ref::<Self>() == Some(self)
  }
}
