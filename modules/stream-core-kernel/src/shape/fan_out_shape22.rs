use super::{Inlet, Outlet, Shape};

#[cfg(test)]
#[path = "fan_out_shape22_test.rs"]
mod tests;

/// Shape with one input port and twenty-two output ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FanOutShape22<
  In,
  Out0,
  Out1,
  Out2,
  Out3,
  Out4,
  Out5,
  Out6,
  Out7,
  Out8,
  Out9,
  Out10,
  Out11,
  Out12,
  Out13,
  Out14,
  Out15,
  Out16,
  Out17,
  Out18,
  Out19,
  Out20,
  Out21,
> {
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
  out11: Outlet<Out11>,
  out12: Outlet<Out12>,
  out13: Outlet<Out13>,
  out14: Outlet<Out14>,
  out15: Outlet<Out15>,
  out16: Outlet<Out16>,
  out17: Outlet<Out17>,
  out18: Outlet<Out18>,
  out19: Outlet<Out19>,
  out20: Outlet<Out20>,
  out21: Outlet<Out21>,
}

impl<
  In,
  Out0,
  Out1,
  Out2,
  Out3,
  Out4,
  Out5,
  Out6,
  Out7,
  Out8,
  Out9,
  Out10,
  Out11,
  Out12,
  Out13,
  Out14,
  Out15,
  Out16,
  Out17,
  Out18,
  Out19,
  Out20,
  Out21,
>
  FanOutShape22<
    In,
    Out0,
    Out1,
    Out2,
    Out3,
    Out4,
    Out5,
    Out6,
    Out7,
    Out8,
    Out9,
    Out10,
    Out11,
    Out12,
    Out13,
    Out14,
    Out15,
    Out16,
    Out17,
    Out18,
    Out19,
    Out20,
    Out21,
  >
{
  /// Creates a new fan-out shape with one inlet and twenty-two outlets.
  #[must_use]
  pub const fn new(
    inlet: Inlet<In>,
    group0: (Outlet<Out0>, Outlet<Out1>, Outlet<Out2>, Outlet<Out3>),
    group1: (Outlet<Out4>, Outlet<Out5>, Outlet<Out6>, Outlet<Out7>),
    group2: (Outlet<Out8>, Outlet<Out9>, Outlet<Out10>, Outlet<Out11>),
    group3: (Outlet<Out12>, Outlet<Out13>, Outlet<Out14>, Outlet<Out15>),
    group4: (Outlet<Out16>, Outlet<Out17>, Outlet<Out18>, Outlet<Out19>),
    group5: (Outlet<Out20>, Outlet<Out21>),
  ) -> Self {
    let (out0, out1, out2, out3) = group0;
    let (out4, out5, out6, out7) = group1;
    let (out8, out9, out10, out11) = group2;
    let (out12, out13, out14, out15) = group3;
    let (out16, out17, out18, out19) = group4;
    let (out20, out21) = group5;
    Self {
      inlet,
      out0,
      out1,
      out2,
      out3,
      out4,
      out5,
      out6,
      out7,
      out8,
      out9,
      out10,
      out11,
      out12,
      out13,
      out14,
      out15,
      out16,
      out17,
      out18,
      out19,
      out20,
      out21,
    }
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

  /// Returns the twelfth output port.
  #[must_use]
  pub const fn out11(&self) -> &Outlet<Out11> {
    &self.out11
  }

  /// Returns the thirteenth output port.
  #[must_use]
  pub const fn out12(&self) -> &Outlet<Out12> {
    &self.out12
  }

  /// Returns the fourteenth output port.
  #[must_use]
  pub const fn out13(&self) -> &Outlet<Out13> {
    &self.out13
  }

  /// Returns the fifteenth output port.
  #[must_use]
  pub const fn out14(&self) -> &Outlet<Out14> {
    &self.out14
  }

  /// Returns the sixteenth output port.
  #[must_use]
  pub const fn out15(&self) -> &Outlet<Out15> {
    &self.out15
  }

  /// Returns the seventeenth output port.
  #[must_use]
  pub const fn out16(&self) -> &Outlet<Out16> {
    &self.out16
  }

  /// Returns the eighteenth output port.
  #[must_use]
  pub const fn out17(&self) -> &Outlet<Out17> {
    &self.out17
  }

  /// Returns the nineteenth output port.
  #[must_use]
  pub const fn out18(&self) -> &Outlet<Out18> {
    &self.out18
  }

  /// Returns the twentieth output port.
  #[must_use]
  pub const fn out19(&self) -> &Outlet<Out19> {
    &self.out19
  }

  /// Returns the twenty-first output port.
  #[must_use]
  pub const fn out20(&self) -> &Outlet<Out20> {
    &self.out20
  }

  /// Returns the twenty-second output port.
  #[must_use]
  pub const fn out21(&self) -> &Outlet<Out21> {
    &self.out21
  }
}

impl<
  In,
  Out0,
  Out1,
  Out2,
  Out3,
  Out4,
  Out5,
  Out6,
  Out7,
  Out8,
  Out9,
  Out10,
  Out11,
  Out12,
  Out13,
  Out14,
  Out15,
  Out16,
  Out17,
  Out18,
  Out19,
  Out20,
  Out21,
> Shape
  for FanOutShape22<
    In,
    Out0,
    Out1,
    Out2,
    Out3,
    Out4,
    Out5,
    Out6,
    Out7,
    Out8,
    Out9,
    Out10,
    Out11,
    Out12,
    Out13,
    Out14,
    Out15,
    Out16,
    Out17,
    Out18,
    Out19,
    Out20,
    Out21,
  >
{
  type In = In;
  type Out = (
    Out0,
    Out1,
    Out2,
    Out3,
    Out4,
    Out5,
    Out6,
    Out7,
    Out8,
    Out9,
    Out10,
    Out11,
    Out12,
    Out13,
    Out14,
    Out15,
    Out16,
    Out17,
    Out18,
    Out19,
    Out20,
    Out21,
  );
}
