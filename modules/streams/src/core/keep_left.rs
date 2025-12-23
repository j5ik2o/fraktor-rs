use super::{MatCombine, MatCombineRule};

/// Keeps the left materialized value.
pub struct KeepLeft;

impl<Left, Right> MatCombineRule<Left, Right> for KeepLeft {
  type Out = Left;

  fn kind() -> MatCombine {
    MatCombine::KeepLeft
  }

  fn combine(left: Left, _right: Right) -> Self::Out {
    left
  }
}
