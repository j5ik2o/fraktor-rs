use alloc::vec::Vec;

use super::{Inlet, Outlet, Shape};

#[cfg(test)]
mod tests;

/// Shape with one input port and multiple uniform output ports.
#[derive(Debug, Clone)]
pub struct UniformFanOutShape<In, Out> {
  inlet:   Inlet<In>,
  outlets: Vec<Outlet<Out>>,
}

impl<In, Out> UniformFanOutShape<In, Out> {
  /// Creates a new fan-out shape from the given inlet and outlets.
  #[must_use]
  pub const fn new(inlet: Inlet<In>, outlets: Vec<Outlet<Out>>) -> Self {
    Self { inlet, outlets }
  }

  /// Creates a fan-out shape with `port_count` outlets and a fresh inlet.
  #[must_use]
  pub fn with_port_count(port_count: usize) -> Self {
    let inlet = Inlet::new();
    let outlets = (0..port_count).map(|_| Outlet::new()).collect();
    Self { inlet, outlets }
  }

  /// Returns the input port.
  #[must_use]
  pub const fn inlet(&self) -> &Inlet<In> {
    &self.inlet
  }

  /// Returns the output ports.
  #[must_use]
  pub fn outlets(&self) -> &[Outlet<Out>] {
    &self.outlets
  }

  /// Returns the number of output ports.
  #[must_use]
  pub const fn port_count(&self) -> usize {
    self.outlets.len()
  }
}

impl<In, Out> Shape for UniformFanOutShape<In, Out> {
  type In = In;
  type Out = Vec<Out>;
}
