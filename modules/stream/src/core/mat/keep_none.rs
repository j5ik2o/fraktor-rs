use super::{StreamNotUsed, mat_combine::MatCombine, mat_combine_rule::MatCombineRule};

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
