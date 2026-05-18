use super::{Inlet, Outlet, Shape};

/// Shape describing a single inlet and outlet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamShape<In, Out> {
  inlet:  Inlet<In>,
  outlet: Outlet<Out>,
}

impl<In, Out> StreamShape<In, Out> {
  /// Creates a new shape.
  #[must_use]
  pub const fn new(inlet: Inlet<In>, outlet: Outlet<Out>) -> Self {
    Self { inlet, outlet }
  }

  /// Returns the inlet.
  #[must_use]
  pub const fn inlet(&self) -> &Inlet<In> {
    &self.inlet
  }

  /// Returns the outlet.
  #[must_use]
  pub const fn outlet(&self) -> &Outlet<Out> {
    &self.outlet
  }
}

impl<In, Out> Shape for StreamShape<In, Out> {
  type In = In;
  type Out = Out;
}
