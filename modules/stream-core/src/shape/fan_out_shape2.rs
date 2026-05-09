use super::{Inlet, Outlet, Shape};

#[cfg(test)]
mod tests;

/// Shape with one input port and two output ports.
///
/// Corresponds to Pekko `FanOutShape2[In, Out0, Out1]` which is used by
/// operators that split a single input into two differently-typed outputs
/// (e.g. `WireTap`, `Unzip`).
///
/// Note: `Partition` uses `UniformFanOutShape` instead, because all its
/// output ports share the same element type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FanOutShape2<In, Out0, Out1> {
  inlet: Inlet<In>,
  out0:  Outlet<Out0>,
  out1:  Outlet<Out1>,
}

impl<In, Out0, Out1> FanOutShape2<In, Out0, Out1> {
  /// Creates a new fan-out shape with one inlet and two outlets.
  #[must_use]
  pub const fn new(inlet: Inlet<In>, out0: Outlet<Out0>, out1: Outlet<Out1>) -> Self {
    Self { inlet, out0, out1 }
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
}

impl<In, Out0, Out1> Shape for FanOutShape2<In, Out0, Out1> {
  type In = In;
  type Out = (Out0, Out1);
}
