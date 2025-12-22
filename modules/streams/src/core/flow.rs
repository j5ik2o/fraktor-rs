//! Flow stage definition.

use crate::core::{
  inlet_id::InletId, outlet_id::OutletId, stage_id::StageId, stream_shape::StreamShape, stream_stage::StreamStage,
};

/// Stream flow stage.
#[derive(Debug, Clone)]
pub struct Flow<In, Out> {
  stage:  StageId,
  inlet:  InletId<In>,
  outlet: OutletId<Out>,
}

impl<In, Out> Flow<In, Out> {
  /// Creates a new flow stage.
  #[must_use]
  pub fn new() -> Self {
    let stage = StageId::next();
    let inlet = InletId::new(stage);
    let outlet = OutletId::new(stage);
    Self { stage, inlet, outlet }
  }

  /// Returns the inlet port identifier.
  #[must_use]
  pub const fn inlet(&self) -> InletId<In> {
    self.inlet
  }

  /// Returns the outlet port identifier.
  #[must_use]
  pub const fn outlet(&self) -> OutletId<Out> {
    self.outlet
  }

  /// Returns the stage identifier.
  #[must_use]
  pub const fn stage_id(&self) -> StageId {
    self.stage
  }
}

impl<In, Out> StreamStage for Flow<In, Out> {
  type In = In;
  type Out = Out;

  fn shape(&self) -> StreamShape {
    StreamShape::Flow
  }

  fn inlet(&self) -> Option<InletId<Self::In>> {
    Some(self.inlet)
  }

  fn outlet(&self) -> Option<OutletId<Self::Out>> {
    Some(self.outlet)
  }
}

impl<In, Out> Default for Flow<In, Out> {
  fn default() -> Self {
    Self::new()
  }
}
