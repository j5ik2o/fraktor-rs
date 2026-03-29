use super::MatCombineRule;
use crate::core::mat::MatCombine;

/// Keeps the left materialized value.
pub struct KeepLeft;

impl<Left, Right> MatCombineRule<Left, Right> for KeepLeft {
  type Out = Left;

  fn kind() -> MatCombine {
    MatCombine::Left
  }

  fn combine(left: Left, _right: Right) -> Self::Out {
    left
  }
}
