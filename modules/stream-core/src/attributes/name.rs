use alloc::{boxed::Box, string::String};
use core::any::Any;

use super::Attribute;

/// Human-readable stage name attribute.
///
/// Mirrors Pekko's `Attributes.Name` case class. The wrapped `String`
/// corresponds to the `n: String` payload of `final case class Name(n: String)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name(pub String);

impl Attribute for Name {
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
