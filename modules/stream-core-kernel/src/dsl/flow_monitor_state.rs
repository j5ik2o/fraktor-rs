//! Stream state tracked by a flow monitor.

#[cfg(test)]
#[path = "flow_monitor_state_test.rs"]
mod tests;

use crate::r#impl::StreamError;

/// Observable state of a monitored flow.
///
/// Corresponds to Pekko's `StreamState[T]` sealed trait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowMonitorState<T> {
  /// The monitor has been created but no element has passed yet.
  Initialized,
  /// The most recently observed element.
  Received(T),
  /// The stream failed with an error.
  Failed(StreamError),
  /// The stream completed normally.
  Finished,
}
