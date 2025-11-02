/// Supervisor strategy variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorStrategyKind {
  /// Only the failing actor is affected.
  OneForOne,
  /// Sibling actors are also restarted when a failure occurs.
  AllForOne,
}
