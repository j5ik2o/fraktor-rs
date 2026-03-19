use super::{MatCombine, MatCombineRule, StreamNotUsed};

/// Drops both materialized values.
pub struct KeepNone;

impl<Left, Right> MatCombineRule<Left, Right> for KeepNone {
  type Out = StreamNotUsed;

  fn kind() -> MatCombine {
    MatCombine::KeepNone
  }

  fn combine(_left: Left, _right: Right) -> Self::Out {
    StreamNotUsed::new()
  }
}
