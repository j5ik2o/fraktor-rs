/// Indicates whether a sink should continue consuming or complete.
#[derive(Debug, PartialEq, Eq)]
pub enum SinkDecision {
  /// Continue processing incoming elements.
  Continue,
  /// Complete the sink immediately.
  Complete,
}
