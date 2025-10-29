//! Supervisor fan-out strategy.

/// Supervisor fan-out strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyKind {
  /// Only the failing child is affected.
  OneForOne,
  /// The entire sibling group responds together.
  AllForOne,
}
