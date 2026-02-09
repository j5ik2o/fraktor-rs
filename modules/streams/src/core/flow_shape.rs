use super::{Inlet, Outlet, Shape};

/// Shape with one input port and one output port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlowShape<In, Out> {
  inlet:  Inlet<In>,
  outlet: Outlet<Out>,
}

impl<In, Out> FlowShape<In, Out> {
  /// Creates a new flow shape.
  #[must_use]
  pub const fn new(inlet: Inlet<In>, outlet: Outlet<Out>) -> Self {
    Self { inlet, outlet }
  }

  /// Returns the input port.
  #[must_use]
  pub const fn inlet(&self) -> &Inlet<In> {
    &self.inlet
  }

  /// Returns the output port.
  #[must_use]
  pub const fn outlet(&self) -> &Outlet<Out> {
    &self.outlet
  }
}

impl<In, Out> Shape for FlowShape<In, Out> {
  type In = In;
  type Out = Out;
}
