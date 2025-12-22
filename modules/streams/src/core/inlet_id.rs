//! Inlet port identifier.

use crate::core::{port_id::PortId, stage_id::StageId};

/// Identifier for an inlet port.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct InletId<T> {
  inner: PortId<T>,
}

impl<T> InletId<T> {
  pub(crate) fn new(stage: StageId) -> Self {
    Self { inner: PortId::new(stage, false) }
  }

  /// Returns the stage identifier that owns this port.
  #[must_use]
  pub const fn stage_id(&self) -> StageId {
    self.inner.stage_id()
  }

  pub(crate) const fn token(&self) -> u64 {
    self.inner.token()
  }
}

impl<T> Copy for InletId<T> {}

impl<T> Clone for InletId<T> {
  fn clone(&self) -> Self {
    *self
  }
}
