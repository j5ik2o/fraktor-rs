use super::{Inlet, Outlet, Shape};

#[cfg(test)]
#[path = "fan_out_shape7_test.rs"]
mod tests;

/// Shape with one input port and seven output ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FanOutShape7<In, Out0, Out1, Out2, Out3, Out4, Out5, Out6> {
  inlet: Inlet<In>,
  out0:  Outlet<Out0>,
  out1:  Outlet<Out1>,
  out2:  Outlet<Out2>,
  out3:  Outlet<Out3>,
  out4:  Outlet<Out4>,
  out5:  Outlet<Out5>,
  out6:  Outlet<Out6>,
}

impl<In, Out0, Out1, Out2, Out3, Out4, Out5, Out6> FanOutShape7<In, Out0, Out1, Out2, Out3, Out4, Out5, Out6> {
  /// Creates a new fan-out shape with one inlet and seven outlets.
  #[must_use]
  pub const fn new(
    inlet: Inlet<In>,
    group0: (Outlet<Out0>, Outlet<Out1>, Outlet<Out2>, Outlet<Out3>),
    group1: (Outlet<Out4>, Outlet<Out5>, Outlet<Out6>),
  ) -> Self {
    let (out0, out1, out2, out3) = group0;
    let (out4, out5, out6) = group1;
    Self { inlet, out0, out1, out2, out3, out4, out5, out6 }
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

  /// Returns the fourth output port.
  #[must_use]
  pub const fn out3(&self) -> &Outlet<Out3> {
    &self.out3
  }

  /// Returns the fifth output port.
  #[must_use]
  pub const fn out4(&self) -> &Outlet<Out4> {
    &self.out4
  }

  /// Returns the sixth output port.
  #[must_use]
  pub const fn out5(&self) -> &Outlet<Out5> {
    &self.out5
  }

  /// Returns the seventh output port.
  #[must_use]
  pub const fn out6(&self) -> &Outlet<Out6> {
    &self.out6
  }
}

impl<In, Out0, Out1, Out2, Out3, Out4, Out5, Out6> Shape
  for FanOutShape7<In, Out0, Out1, Out2, Out3, Out4, Out5, Out6>
{
  type In = In;
  type Out = (Out0, Out1, Out2, Out3, Out4, Out5, Out6);
}
