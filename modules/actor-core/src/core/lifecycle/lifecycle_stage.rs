//! Lifecycle stage enumeration.

/// Lifecycle stage transitions captured for observability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleStage {
  /// Actor has started.
  Started,
  /// Actor has restarted following a failure.
  Restarted,
  /// Actor has stopped.
  Stopped,
}
