/// Execution modes used to interpret batch data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchMode {
  /// Single-shot execution.
  OneShot,
  /// Fixed-rate periodic execution.
  FixedRate,
  /// Fixed-delay periodic execution.
  FixedDelay,
}
