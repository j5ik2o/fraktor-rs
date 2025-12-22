//! Sink stage definition.

use crate::core::{
  inlet_id::InletId, outlet_id::OutletId, stage_id::StageId, stream_shape::StreamShape, stream_stage::StreamStage,
};

/// Stream sink stage.
#[derive(Debug, Clone)]
pub struct Sink<In> {
  stage: StageId,
  inlet: InletId<In>,
}

impl<In> Sink<In> {
  /// Creates a new sink stage.
  #[must_use]
  pub fn new() -> Self {
    let stage = StageId::next();
    let inlet = InletId::new(stage);
    Self { stage, inlet }
  }

  /// Returns the inlet port identifier.
  #[must_use]
  pub const fn inlet(&self) -> InletId<In> {
    self.inlet
  }

  /// Returns the stage identifier.
  #[must_use]
  pub const fn stage_id(&self) -> StageId {
    self.stage
  }
}

impl<In> StreamStage for Sink<In> {
  type In = In;
  type Out = ();

  fn shape(&self) -> StreamShape {
    StreamShape::Sink
  }

  fn inlet(&self) -> Option<InletId<Self::In>> {
    Some(self.inlet)
  }

  fn outlet(&self) -> Option<OutletId<Self::Out>> {
    None
  }
}

impl<In> Default for Sink<In> {
  fn default() -> Self {
    Self::new()
  }
}
