use super::{Outlet, Shape};

/// Shape with one output port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceShape<Out> {
  outlet: Outlet<Out>,
}

impl<Out> SourceShape<Out> {
  /// Creates a new source shape.
  #[must_use]
  pub const fn new(outlet: Outlet<Out>) -> Self {
    Self { outlet }
  }

  /// Returns the output port.
  #[must_use]
  pub const fn outlet(&self) -> &Outlet<Out> {
    &self.outlet
  }
}

impl<Out> Shape for SourceShape<Out> {
  type In = ();
  type Out = Out;
}
