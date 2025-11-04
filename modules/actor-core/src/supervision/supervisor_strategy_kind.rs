//! Supervisor strategy variants.

/// Supervisor strategy variants describing how siblings are affected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorStrategyKind {
  /// Only the failing actor is affected by the directive.
  OneForOne,
  /// All siblings of the failing actor are affected equally.
  AllForOne,
}
