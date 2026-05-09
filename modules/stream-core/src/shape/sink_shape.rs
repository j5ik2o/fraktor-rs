use super::{Inlet, Shape};

/// Shape with one input port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SinkShape<In> {
  inlet: Inlet<In>,
}

impl<In> SinkShape<In> {
  /// Creates a new sink shape.
  #[must_use]
  pub const fn new(inlet: Inlet<In>) -> Self {
    Self { inlet }
  }

  /// Returns the input port.
  #[must_use]
  pub const fn inlet(&self) -> &Inlet<In> {
    &self.inlet
  }
}

impl<In> Shape for SinkShape<In> {
  type In = In;
  type Out = ();
}
