use super::MatCombineRule;
use crate::core::{StreamNotUsed, mat::MatCombine};

/// Drops both materialized values.
pub struct KeepNone;

impl<Left, Right> MatCombineRule<Left, Right> for KeepNone {
  type Out = StreamNotUsed;

  fn kind() -> MatCombine {
    MatCombine::Neither
  }

  fn combine(_left: Left, _right: Right) -> Self::Out {
    StreamNotUsed::new()
  }
}
