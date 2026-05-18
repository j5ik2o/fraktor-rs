//! Default flow monitor implementation.

use super::{FlowMonitor, FlowMonitorState};

/// Default flow monitor implementation using direct state storage.
///
/// Uses `&mut self` for state updates following the project's
/// immutability policy (prefer `&mut self` over internal mutability).
#[derive(Debug, PartialEq, Eq)]
pub struct FlowMonitorImpl<Out> {
  current: FlowMonitorState<Out>,
}

impl<Out> FlowMonitorImpl<Out> {
  /// Creates a new monitor in the [`FlowMonitorState::Initialized`] state.
  #[must_use]
  pub const fn new() -> Self {
    Self { current: FlowMonitorState::Initialized }
  }

  /// Updates the current state.
  pub fn set_state(&mut self, state: FlowMonitorState<Out>) {
    self.current = state;
  }
}

impl<Out> Default for FlowMonitorImpl<Out> {
  fn default() -> Self {
    Self::new()
  }
}

impl<Out: Clone> FlowMonitor<Out> for FlowMonitorImpl<Out> {
  fn state(&self) -> FlowMonitorState<Out> {
    self.current.clone()
  }
}
