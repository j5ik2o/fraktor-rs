/// Outcome produced by a stack push operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushOutcome {
  /// The element was successfully pushed.
  Pushed,
  /// The stack grew to accommodate additional elements.
  GrewTo {
    /// New capacity after the storage has grown.
    capacity: usize,
  },
}
