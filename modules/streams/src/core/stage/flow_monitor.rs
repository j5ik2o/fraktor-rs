use core::marker::PhantomData;

/// Materialized monitor handle for a flow output stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowMonitor<Out> {
  _pd: PhantomData<fn() -> Out>,
}

impl<Out> FlowMonitor<Out> {
  /// Creates a new flow monitor handle.
  #[must_use]
  pub const fn new() -> Self {
    Self { _pd: PhantomData }
  }
}

impl<Out> Default for FlowMonitor<Out> {
  fn default() -> Self {
    Self::new()
  }
}
