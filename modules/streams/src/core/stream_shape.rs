//! Stream stage shape definitions.

/// Shape of a stream stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamShape {
  /// Source stage with only an outlet.
  Source,
  /// Flow stage with inlet and outlet.
  Flow,
  /// Sink stage with only an inlet.
  Sink,
}
