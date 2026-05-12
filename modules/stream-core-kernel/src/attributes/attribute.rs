#[cfg(test)]
#[path = "attribute_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::{any::Any, fmt::Debug};

/// Marker trait for typed stream attributes.
///
/// Each attribute type can be stored in an [`Attributes`] collection
/// and retrieved by its concrete type via downcasting.
pub trait Attribute: Any + Send + Sync + Debug {
  /// Returns a reference to the underlying `Any` for downcasting.
  fn as_any(&self) -> &dyn Any;

  /// Clones this attribute into a new boxed trait object.
  fn clone_box(&self) -> Box<dyn Attribute>;

  /// Compares this attribute with another for equality via downcasting.
  fn eq_attr(&self, other: &dyn Any) -> bool;
}
