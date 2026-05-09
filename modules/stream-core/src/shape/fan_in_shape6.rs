use super::{Inlet, Outlet, Shape};

#[cfg(test)]
mod tests;

/// Shape with six input ports and one output port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FanInShape6<In0, In1, In2, In3, In4, In5, Out> {
  in0: Inlet<In0>,
  in1: Inlet<In1>,
  in2: Inlet<In2>,
  in3: Inlet<In3>,
  in4: Inlet<In4>,
  in5: Inlet<In5>,
  out: Outlet<Out>,
}

impl<In0, In1, In2, In3, In4, In5, Out> FanInShape6<In0, In1, In2, In3, In4, In5, Out> {
  /// Creates a new fan-in shape with six inlets and one outlet.
  #[must_use]
  pub const fn new(
    in0: Inlet<In0>,
    in1: Inlet<In1>,
    in2: Inlet<In2>,
    in3: Inlet<In3>,
    in4: Inlet<In4>,
    in5: Inlet<In5>,
    out: Outlet<Out>,
  ) -> Self {
    Self { in0, in1, in2, in3, in4, in5, out }
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

  /// Returns the fifth input port.
  #[must_use]
  pub const fn in4(&self) -> &Inlet<In4> {
    &self.in4
  }

  /// Returns the sixth input port.
  #[must_use]
  pub const fn in5(&self) -> &Inlet<In5> {
    &self.in5
  }

  /// Returns the output port.
  #[must_use]
  pub const fn out(&self) -> &Outlet<Out> {
    &self.out
  }
}

impl<In0, In1, In2, In3, In4, In5, Out> Shape for FanInShape6<In0, In1, In2, In3, In4, In5, Out> {
  type In = (In0, In1, In2, In3, In4, In5);
  type Out = Out;
}
