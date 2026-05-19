/// Base trait for stream topology shapes.
pub trait Shape {
  /// Input-side type represented by this shape.
  type In;
  /// Output-side type represented by this shape.
  type Out;
}
