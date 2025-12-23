use core::marker::PhantomData;

use super::PortId;

/// Typed inlet port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Inlet<T> {
  id:  PortId,
  _pd: PhantomData<fn() -> T>,
}

impl<T> Inlet<T> {
  /// Creates a new inlet.
  #[must_use]
  pub fn new() -> Self {
    Self { id: PortId::next(), _pd: PhantomData }
  }

  /// Returns the port identifier.
  #[must_use]
  pub const fn id(&self) -> PortId {
    self.id
  }

  pub(crate) const fn from_id(id: PortId) -> Self {
    Self { id, _pd: PhantomData }
  }
}

impl<T> Default for Inlet<T> {
  fn default() -> Self {
    Self::new()
  }
}
