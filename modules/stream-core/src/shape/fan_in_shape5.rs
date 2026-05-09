use super::{Inlet, Outlet, Shape};

#[cfg(test)]
mod tests;

/// Shape with five input ports and one output port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FanInShape5<In0, In1, In2, In3, In4, Out> {
  in0: Inlet<In0>,
  in1: Inlet<In1>,
  in2: Inlet<In2>,
  in3: Inlet<In3>,
  in4: Inlet<In4>,
  out: Outlet<Out>,
}

impl<In0, In1, In2, In3, In4, Out> FanInShape5<In0, In1, In2, In3, In4, Out> {
  /// Creates a new fan-in shape with five inlets and one outlet.
  #[must_use]
  pub const fn new(
    in0: Inlet<In0>,
    in1: Inlet<In1>,
    in2: Inlet<In2>,
    in3: Inlet<In3>,
    in4: Inlet<In4>,
    out: Outlet<Out>,
  ) -> Self {
    Self { in0, in1, in2, in3, in4, out }
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

  /// Returns the output port.
  #[must_use]
  pub const fn out(&self) -> &Outlet<Out> {
    &self.out
  }
}

impl<In0, In1, In2, In3, In4, Out> Shape for FanInShape5<In0, In1, In2, In3, In4, Out> {
  type In = (In0, In1, In2, In3, In4);
  type Out = Out;
}
