use super::MatCombineRule;
use crate::core::mat::MatCombine;

/// Keeps both materialized values.
pub struct KeepBoth;

impl<Left, Right> MatCombineRule<Left, Right> for KeepBoth {
  type Out = (Left, Right);

  fn kind() -> MatCombine {
    MatCombine::Both
  }

  fn combine(left: Left, right: Right) -> Self::Out {
    (left, right)
  }
}
