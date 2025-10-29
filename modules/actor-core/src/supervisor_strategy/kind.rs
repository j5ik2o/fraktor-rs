/// Supervisor coordination style.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrategyKind {
  /// Only the failing child is affected.
  OneForOne,
  /// The entire sibling group is treated together.
  AllForOne,
}
