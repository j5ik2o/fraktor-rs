use super::{Inlet, Outlet, Shape};

#[cfg(test)]
mod tests;

/// Shape with one input port and one output port.
///
/// Corresponds to Pekko `FanInShape1[-T0, +O]`. Although topologically
/// identical to `FlowShape`, Pekko keeps the type distinct as the base
/// case of the `FanInShape` family. Because only a single inlet exists,
/// [`Shape::In`] is exposed unwrapped rather than as a single-element
/// tuple (contrast with `FanInShape2`, which uses `(In0, In1)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FanInShape1<In0, Out> {
  in0: Inlet<In0>,
  out: Outlet<Out>,
}

impl<In0, Out> FanInShape1<In0, Out> {
  /// Creates a new fan-in shape with a single inlet and outlet.
  #[must_use]
  pub const fn new(in0: Inlet<In0>, out: Outlet<Out>) -> Self {
    Self { in0, out }
  }

  /// Returns the input port.
  #[must_use]
  pub const fn in0(&self) -> &Inlet<In0> {
    &self.in0
  }

  /// Returns the output port.
  #[must_use]
  pub const fn out(&self) -> &Outlet<Out> {
    &self.out
  }
}

impl<In0, Out> Shape for FanInShape1<In0, Out> {
  type In = In0;
  type Out = Out;
}
