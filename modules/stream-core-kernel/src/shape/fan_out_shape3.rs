use super::{Inlet, Outlet, Shape};

#[cfg(test)]
#[path = "fan_out_shape3_test.rs"]
mod tests;

/// Shape with one input port and three output ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FanOutShape3<In, Out0, Out1, Out2> {
  inlet: Inlet<In>,
  out0:  Outlet<Out0>,
  out1:  Outlet<Out1>,
  out2:  Outlet<Out2>,
}

impl<In, Out0, Out1, Out2> FanOutShape3<In, Out0, Out1, Out2> {
  /// Creates a new fan-out shape with one inlet and three outlets.
  #[must_use]
  pub const fn new(inlet: Inlet<In>, out0: Outlet<Out0>, out1: Outlet<Out1>, out2: Outlet<Out2>) -> Self {
    Self { inlet, out0, out1, out2 }
  }

  /// Returns the input port.
  #[must_use]
  pub const fn inlet(&self) -> &Inlet<In> {
    &self.inlet
  }

  /// Returns the first output port.
  #[must_use]
  pub const fn out0(&self) -> &Outlet<Out0> {
    &self.out0
  }

  /// Returns the second output port.
  #[must_use]
  pub const fn out1(&self) -> &Outlet<Out1> {
    &self.out1
  }

  /// Returns the third output port.
  #[must_use]
  pub const fn out2(&self) -> &Outlet<Out2> {
    &self.out2
  }
}

impl<In, Out0, Out1, Out2> Shape for FanOutShape3<In, Out0, Out1, Out2> {
  type In = In;
  type Out = (Out0, Out1, Out2);
}
