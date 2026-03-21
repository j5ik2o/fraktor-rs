/// Cancellation handling for `split_when` and `split_after` substreams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubstreamCancelStrategy {
  /// Consume the remaining upstream elements instead of cancelling upstream immediately.
  Drain,
  /// Propagate cancellation to the upstream side.
  Propagate,
}
