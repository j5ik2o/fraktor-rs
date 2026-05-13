use super::{Inlet, Outlet, Shape};

#[cfg(test)]
#[path = "fan_out_shape6_test.rs"]
mod tests;

/// Shape with one input port and six output ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FanOutShape6<In, Out0, Out1, Out2, Out3, Out4, Out5> {
  inlet: Inlet<In>,
  out0:  Outlet<Out0>,
  out1:  Outlet<Out1>,
  out2:  Outlet<Out2>,
  out3:  Outlet<Out3>,
  out4:  Outlet<Out4>,
  out5:  Outlet<Out5>,
}

impl<In, Out0, Out1, Out2, Out3, Out4, Out5> FanOutShape6<In, Out0, Out1, Out2, Out3, Out4, Out5> {
  /// Creates a new fan-out shape with one inlet and six outlets.
  #[must_use]
  pub const fn new(
    inlet: Inlet<In>,
    out0: Outlet<Out0>,
    out1: Outlet<Out1>,
    out2: Outlet<Out2>,
    out3: Outlet<Out3>,
    out4: Outlet<Out4>,
    out5: Outlet<Out5>,
  ) -> Self {
    Self { inlet, out0, out1, out2, out3, out4, out5 }
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
}

impl<In, Out0, Out1, Out2, Out3, Out4, Out5> Shape for FanOutShape6<In, Out0, Out1, Out2, Out3, Out4, Out5> {
  type In = In;
  type Out = (Out0, Out1, Out2, Out3, Out4, Out5);
}
