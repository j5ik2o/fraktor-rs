use super::{Inlet, Outlet, Shape};

#[cfg(test)]
#[path = "fan_in_shape3_test.rs"]
mod tests;

/// Shape with three input ports and one output port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FanInShape3<In0, In1, In2, Out> {
  in0: Inlet<In0>,
  in1: Inlet<In1>,
  in2: Inlet<In2>,
  out: Outlet<Out>,
}

impl<In0, In1, In2, Out> FanInShape3<In0, In1, In2, Out> {
  /// Creates a new fan-in shape with three inlets and one outlet.
  #[must_use]
  pub const fn new(in0: Inlet<In0>, in1: Inlet<In1>, in2: Inlet<In2>, out: Outlet<Out>) -> Self {
    Self { in0, in1, in2, out }
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

  /// Returns the output port.
  #[must_use]
  pub const fn out(&self) -> &Outlet<Out> {
    &self.out
  }
}

impl<In0, In1, In2, Out> Shape for FanInShape3<In0, In1, In2, Out> {
  type In = (In0, In1, In2);
  type Out = Out;
}
