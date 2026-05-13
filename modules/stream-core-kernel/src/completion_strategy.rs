#[cfg(test)]
#[path = "completion_strategy_test.rs"]
mod tests;

/// Completion strategy for actor-sourced streams.
///
/// Determines how an actor source handles completion when signalled
/// by the external producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionStrategy {
  /// Complete immediately, discarding buffered elements.
  Immediately,
  /// Drain buffered elements before completing.
  Draining,
}
