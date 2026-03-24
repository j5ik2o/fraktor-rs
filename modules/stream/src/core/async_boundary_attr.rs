//! Marker attribute for async boundaries in stream graphs.

use core::any::Any;

use super::Attribute;

/// Marker attribute indicating an async boundary.
///
/// When present on a graph node, the materializer may split the graph
/// into separate islands at that point.  This type mirrors Pekko's
/// `Attributes.AsyncBoundary` case object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsyncBoundaryAttr;

impl Attribute for AsyncBoundaryAttr {
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn clone_box(&self) -> alloc::boxed::Box<dyn Attribute> {
    alloc::boxed::Box::new(*self)
  }

  fn eq_attr(&self, other: &dyn Any) -> bool {
    other.downcast_ref::<Self>() == Some(self)
  }
}
