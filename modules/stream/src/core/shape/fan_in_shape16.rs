use super::{Inlet, Outlet, Shape};

#[cfg(test)]
mod tests;

/// Shape with sixteen input ports and one output port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FanInShape16<In0, In1, In2, In3, In4, In5, In6, In7, In8, In9, In10, In11, In12, In13, In14, In15, Out> {
  in0:  Inlet<In0>,
  in1:  Inlet<In1>,
  in2:  Inlet<In2>,
  in3:  Inlet<In3>,
  in4:  Inlet<In4>,
  in5:  Inlet<In5>,
  in6:  Inlet<In6>,
  in7:  Inlet<In7>,
  in8:  Inlet<In8>,
  in9:  Inlet<In9>,
  in10: Inlet<In10>,
  in11: Inlet<In11>,
  in12: Inlet<In12>,
  in13: Inlet<In13>,
  in14: Inlet<In14>,
  in15: Inlet<In15>,
  out:  Outlet<Out>,
}

impl<In0, In1, In2, In3, In4, In5, In6, In7, In8, In9, In10, In11, In12, In13, In14, In15, Out>
  FanInShape16<In0, In1, In2, In3, In4, In5, In6, In7, In8, In9, In10, In11, In12, In13, In14, In15, Out>
{
  /// Creates a new fan-in shape with sixteen inlets and one outlet.
  #[must_use]
  #[allow(clippy::too_many_arguments)]
  pub const fn new(
    in0: Inlet<In0>,
    in1: Inlet<In1>,
    in2: Inlet<In2>,
    in3: Inlet<In3>,
    in4: Inlet<In4>,
    in5: Inlet<In5>,
    in6: Inlet<In6>,
    in7: Inlet<In7>,
    in8: Inlet<In8>,
    in9: Inlet<In9>,
    in10: Inlet<In10>,
    in11: Inlet<In11>,
    in12: Inlet<In12>,
    in13: Inlet<In13>,
    in14: Inlet<In14>,
    in15: Inlet<In15>,
    out: Outlet<Out>,
  ) -> Self {
    Self { in0, in1, in2, in3, in4, in5, in6, in7, in8, in9, in10, in11, in12, in13, in14, in15, out }
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

  /// Returns the seventh input port.
  #[must_use]
  pub const fn in6(&self) -> &Inlet<In6> {
    &self.in6
  }

  /// Returns the eighth input port.
  #[must_use]
  pub const fn in7(&self) -> &Inlet<In7> {
    &self.in7
  }

  /// Returns the ninth input port.
  #[must_use]
  pub const fn in8(&self) -> &Inlet<In8> {
    &self.in8
  }

  /// Returns the tenth input port.
  #[must_use]
  pub const fn in9(&self) -> &Inlet<In9> {
    &self.in9
  }

  /// Returns the eleventh input port.
  #[must_use]
  pub const fn in10(&self) -> &Inlet<In10> {
    &self.in10
  }

  /// Returns the twelfth input port.
  #[must_use]
  pub const fn in11(&self) -> &Inlet<In11> {
    &self.in11
  }

  /// Returns the thirteenth input port.
  #[must_use]
  pub const fn in12(&self) -> &Inlet<In12> {
    &self.in12
  }

  /// Returns the fourteenth input port.
  #[must_use]
  pub const fn in13(&self) -> &Inlet<In13> {
    &self.in13
  }

  /// Returns the fifteenth input port.
  #[must_use]
  pub const fn in14(&self) -> &Inlet<In14> {
    &self.in14
  }

  /// Returns the sixteenth input port.
  #[must_use]
  pub const fn in15(&self) -> &Inlet<In15> {
    &self.in15
  }

  /// Returns the output port.
  #[must_use]
  pub const fn out(&self) -> &Outlet<Out> {
    &self.out
  }
}

impl<In0, In1, In2, In3, In4, In5, In6, In7, In8, In9, In10, In11, In12, In13, In14, In15, Out> Shape
  for FanInShape16<In0, In1, In2, In3, In4, In5, In6, In7, In8, In9, In10, In11, In12, In13, In14, In15, Out>
{
  type In = (In0, In1, In2, In3, In4, In5, In6, In7, In8, In9, In10, In11, In12, In13, In14, In15);
  type Out = Out;
}
