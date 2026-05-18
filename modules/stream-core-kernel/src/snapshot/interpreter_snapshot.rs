use crate::snapshot::LogicSnapshot;

/// Read-only view over an interpreter's stage list.
///
/// Corresponds to Pekko `pekko.stream.snapshot.InterpreterSnapshot`, whose
/// sole public contract is exposing the collection of `LogicSnapshot`
/// instances contained in the interpreter.
///
/// The trait is object-safe so that heterogeneous interpreter snapshots can
/// be stored behind `Box<dyn InterpreterSnapshot>` (and eventually plugged
/// into `StreamSnapshot` once additional interpreter kinds are modelled).
pub trait InterpreterSnapshot {
  /// Returns the stage logics captured by this snapshot.
  fn logics(&self) -> &[LogicSnapshot];
}
