use super::{Inlet, Outlet, Shape};

#[cfg(test)]
#[path = "fan_in_shape4_test.rs"]
mod tests;

/// Shape with four input ports and one output port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FanInShape4<In0, In1, In2, In3, Out> {
  in0: Inlet<In0>,
  in1: Inlet<In1>,
  in2: Inlet<In2>,
  in3: Inlet<In3>,
  out: Outlet<Out>,
}

impl<In0, In1, In2, In3, Out> FanInShape4<In0, In1, In2, In3, Out> {
  /// Creates a new fan-in shape with four inlets and one outlet.
  #[must_use]
  pub const fn new(in0: Inlet<In0>, in1: Inlet<In1>, in2: Inlet<In2>, in3: Inlet<In3>, out: Outlet<Out>) -> Self {
    Self { in0, in1, in2, in3, out }
  }

  /// Returns the first input port.
  #[must_use]
  pub const fn in0(&self) -> &Inlet<In0> {
    &self.in0
  }

  /// Returns the second input port.
  #[must_use]
  pub const fn in1(&self) -> &Inlet<In1> {
    &self.in1
  }

  /// Returns the third input port.
  #[must_use]
  pub const fn in2(&self) -> &Inlet<In2> {
    &self.in2
  }

  /// Returns the fourth input port.
  #[must_use]
  pub const fn in3(&self) -> &Inlet<In3> {
    &self.in3
  }

  /// Returns the output port.
  #[must_use]
  pub const fn out(&self) -> &Outlet<Out> {
    &self.out
  }
}

impl<In0, In1, In2, In3, Out> Shape for FanInShape4<In0, In1, In2, In3, Out> {
  type In = (In0, In1, In2, In3);
  type Out = Out;
}
