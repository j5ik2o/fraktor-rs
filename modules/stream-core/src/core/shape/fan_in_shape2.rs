use super::{Inlet, Outlet, Shape};

#[cfg(test)]
mod tests;

/// Shape with two input ports and one output port.
///
/// Corresponds to Pekko `FanInShape2[In0, In1, Out]` which is used by
/// operators that merge two differently-typed inputs into a single output
/// (e.g. `MergeSorted`, `ZipWith`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FanInShape2<In0, In1, Out> {
  in0: Inlet<In0>,
  in1: Inlet<In1>,
  out: Outlet<Out>,
}

impl<In0, In1, Out> FanInShape2<In0, In1, Out> {
  /// Creates a new fan-in shape with two inlets and one outlet.
  #[must_use]
  pub const fn new(in0: Inlet<In0>, in1: Inlet<In1>, out: Outlet<Out>) -> Self {
    Self { in0, in1, out }
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

  /// Returns the output port.
  #[must_use]
  pub const fn out(&self) -> &Outlet<Out> {
    &self.out
  }
}

impl<In0, In1, Out> Shape for FanInShape2<In0, In1, Out> {
  type In = (In0, In1);
  type Out = Out;
}
