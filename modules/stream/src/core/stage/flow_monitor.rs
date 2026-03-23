//! Flow monitor trait definition.

#[cfg(test)]
mod tests;

use super::FlowMonitorState;

/// Observable handle for tracking the state of a flow.
///
/// Corresponds to Pekko's `FlowMonitor[T]` trait.
pub trait FlowMonitor<Out> {
  /// Returns the current observed state.
  fn state(&self) -> FlowMonitorState<Out>
  where
    Out: Clone;
}
