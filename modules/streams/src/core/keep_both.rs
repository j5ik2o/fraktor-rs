use super::{MatCombine, MatCombineRule};

/// Keeps both materialized values.
pub struct KeepBoth;

impl<Left, Right> MatCombineRule<Left, Right> for KeepBoth {
  type Out = (Left, Right);

  fn kind() -> MatCombine {
    MatCombine::KeepBoth
  }

  fn combine(left: Left, right: Right) -> Self::Out {
    (left, right)
  }
}
