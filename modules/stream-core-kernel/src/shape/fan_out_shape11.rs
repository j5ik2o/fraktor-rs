use super::{Inlet, Outlet, Shape};

#[cfg(test)]
#[path = "fan_out_shape11_test.rs"]
mod tests;

/// Shape with one input port and eleven output ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FanOutShape11<In, Out0, Out1, Out2, Out3, Out4, Out5, Out6, Out7, Out8, Out9, Out10> {
  inlet: Inlet<In>,
  out0:  Outlet<Out0>,
  out1:  Outlet<Out1>,
  out2:  Outlet<Out2>,
  out3:  Outlet<Out3>,
  out4:  Outlet<Out4>,
  out5:  Outlet<Out5>,
  out6:  Outlet<Out6>,
  out7:  Outlet<Out7>,
  out8:  Outlet<Out8>,
  out9:  Outlet<Out9>,
  out10: Outlet<Out10>,
}

impl<In, Out0, Out1, Out2, Out3, Out4, Out5, Out6, Out7, Out8, Out9, Out10>
  FanOutShape11<In, Out0, Out1, Out2, Out3, Out4, Out5, Out6, Out7, Out8, Out9, Out10>
{
  /// Creates a new fan-out shape with one inlet and eleven outlets.
  #[must_use]
  pub const fn new(
    inlet: Inlet<In>,
    group0: (Outlet<Out0>, Outlet<Out1>, Outlet<Out2>, Outlet<Out3>),
    group1: (Outlet<Out4>, Outlet<Out5>, Outlet<Out6>, Outlet<Out7>),
    group2: (Outlet<Out8>, Outlet<Out9>, Outlet<Out10>),
  ) -> Self {
    let (out0, out1, out2, out3) = group0;
    let (out4, out5, out6, out7) = group1;
    let (out8, out9, out10) = group2;
    Self { inlet, out0, out1, out2, out3, out4, out5, out6, out7, out8, out9, out10 }
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

  /// Returns the eighth output port.
  #[must_use]
  pub const fn out7(&self) -> &Outlet<Out7> {
    &self.out7
  }

  /// Returns the ninth output port.
  #[must_use]
  pub const fn out8(&self) -> &Outlet<Out8> {
    &self.out8
  }

  /// Returns the tenth output port.
  #[must_use]
  pub const fn out9(&self) -> &Outlet<Out9> {
    &self.out9
  }

  /// Returns the eleventh output port.
  #[must_use]
  pub const fn out10(&self) -> &Outlet<Out10> {
    &self.out10
  }
}

impl<In, Out0, Out1, Out2, Out3, Out4, Out5, Out6, Out7, Out8, Out9, Out10> Shape
  for FanOutShape11<In, Out0, Out1, Out2, Out3, Out4, Out5, Out6, Out7, Out8, Out9, Out10>
{
  type In = In;
  type Out = (Out0, Out1, Out2, Out3, Out4, Out5, Out6, Out7, Out8, Out9, Out10);
}
