//! Outlet port identifier.

use crate::core::{port_id::PortId, stage_id::StageId};

/// Identifier for an outlet port.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct OutletId<T> {
  inner: PortId<T>,
}

impl<T> OutletId<T> {
  pub(crate) fn new(stage: StageId) -> Self {
    Self { inner: PortId::new(stage, true) }
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

impl<T> Copy for OutletId<T> {}

impl<T> Clone for OutletId<T> {
  fn clone(&self) -> Self {
    *self
  }
}
