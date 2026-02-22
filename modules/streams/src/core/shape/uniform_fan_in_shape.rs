use alloc::vec::Vec;

use super::{Inlet, Outlet, Shape};

/// Shape with multiple uniform input ports and one output port.
///
/// Corresponds to Pekko `UniformFanInShape[T, O]` which has N inlets of the
/// same type `In` and a single outlet of type `Out`.
#[derive(Debug, Clone)]
pub struct UniformFanInShape<In, Out> {
  inlets: Vec<Inlet<In>>,
  outlet: Outlet<Out>,
}

impl<In, Out> UniformFanInShape<In, Out> {
  /// Creates a new fan-in shape from the given inlets and outlet.
  #[must_use]
  pub fn new(inlets: Vec<Inlet<In>>, outlet: Outlet<Out>) -> Self {
    Self { inlets, outlet }
  }

  /// Creates a fan-in shape with `port_count` inlets and a fresh outlet.
  #[must_use]
  pub fn with_port_count(port_count: usize) -> Self {
    let inlets = (0..port_count).map(|_| Inlet::new()).collect();
    let outlet = Outlet::new();
    Self { inlets, outlet }
  }

  /// Returns the input ports.
  #[must_use]
  pub fn inlets(&self) -> &[Inlet<In>] {
    &self.inlets
  }

  /// Returns the output port.
  #[must_use]
  pub fn outlet(&self) -> &Outlet<Out> {
    &self.outlet
  }

  /// Returns the number of input ports.
  #[must_use]
  pub fn port_count(&self) -> usize {
    self.inlets.len()
  }
}

impl<In, Out> Shape for UniformFanInShape<In, Out> {
  type In = In;
  type Out = Out;
}
