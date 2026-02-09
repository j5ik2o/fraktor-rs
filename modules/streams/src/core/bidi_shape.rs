use super::{Inlet, Outlet, Shape};

/// Shape with two input ports and two output ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BidiShape<In1, Out1, In2, Out2> {
  top_inlet:     Inlet<In1>,
  top_outlet:    Outlet<Out1>,
  bottom_inlet:  Inlet<In2>,
  bottom_outlet: Outlet<Out2>,
}

impl<In1, Out1, In2, Out2> BidiShape<In1, Out1, In2, Out2> {
  /// Creates a new bidirectional shape.
  #[must_use]
  pub const fn new(
    top_inlet: Inlet<In1>,
    top_outlet: Outlet<Out1>,
    bottom_inlet: Inlet<In2>,
    bottom_outlet: Outlet<Out2>,
  ) -> Self {
    Self { top_inlet, top_outlet, bottom_inlet, bottom_outlet }
  }

  /// Returns the top input port.
  #[must_use]
  pub const fn top_inlet(&self) -> &Inlet<In1> {
    &self.top_inlet
  }

  /// Returns the top output port.
  #[must_use]
  pub const fn top_outlet(&self) -> &Outlet<Out1> {
    &self.top_outlet
  }

  /// Returns the bottom input port.
  #[must_use]
  pub const fn bottom_inlet(&self) -> &Inlet<In2> {
    &self.bottom_inlet
  }

  /// Returns the bottom output port.
  #[must_use]
  pub const fn bottom_outlet(&self) -> &Outlet<Out2> {
    &self.bottom_outlet
  }
}

impl<In1, Out1, In2, Out2> Shape for BidiShape<In1, Out1, In2, Out2> {
  type In = (In1, In2);
  type Out = (Out1, Out2);
}
