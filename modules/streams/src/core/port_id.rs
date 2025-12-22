//! Port identifier shared between inlet and outlet ports.

use core::marker::PhantomData;

use crate::core::stage_id::StageId;

/// Identifier shared by stream ports.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct PortId<T> {
  token:   u64,
  stage:   StageId,
  _marker: PhantomData<T>,
}

impl<T> PortId<T> {
  pub(crate) fn new(stage: StageId, is_outlet: bool) -> Self {
    let token = (stage.value() << 1) | u64::from(is_outlet);
    Self { token, stage, _marker: PhantomData }
  }

  /// Returns the stage identifier that owns this port.
  #[must_use]
  pub const fn stage_id(&self) -> StageId {
    self.stage
  }

  pub(crate) const fn token(&self) -> u64 {
    self.token
  }
}

impl<T> Copy for PortId<T> {}

impl<T> Clone for PortId<T> {
  fn clone(&self) -> Self {
    *self
  }
}
