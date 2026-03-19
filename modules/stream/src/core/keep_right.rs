use super::{MatCombine, MatCombineRule};

/// Keeps the right materialized value.
pub struct KeepRight;

impl<Left, Right> MatCombineRule<Left, Right> for KeepRight {
  type Out = Right;

  fn kind() -> MatCombine {
    MatCombine::KeepRight
  }

  fn combine(_left: Left, right: Right) -> Self::Out {
    right
  }
}
