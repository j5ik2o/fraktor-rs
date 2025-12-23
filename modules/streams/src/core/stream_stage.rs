use super::StreamShape;

/// Trait implemented by stream stages.
pub trait StreamStage {
  /// Input type.
  type In;
  /// Output type.
  type Out;

  /// Returns the stage shape.
  fn shape(&self) -> StreamShape<Self::In, Self::Out>;
}
