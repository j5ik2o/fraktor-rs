//! Stream stage trait definitions.

use crate::core::{inlet_id::InletId, outlet_id::OutletId, stream_shape::StreamShape};

/// Common interface for stream stages.
pub trait StreamStage {
  /// Input type handled by the stage.
  type In;
  /// Output type produced by the stage.
  type Out;

  /// Returns the stage shape.
  fn shape(&self) -> StreamShape;
  /// Returns the inlet port, if any.
  fn inlet(&self) -> Option<InletId<Self::In>>;
  /// Returns the outlet port, if any.
  fn outlet(&self) -> Option<OutletId<Self::Out>>;
}
