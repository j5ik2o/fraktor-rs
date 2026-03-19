use core::marker::PhantomData;

use super::PortId;

/// Typed outlet port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Outlet<T> {
  id:  PortId,
  _pd: PhantomData<fn() -> T>,
}

impl<T> Outlet<T> {
  /// Creates a new outlet.
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

impl<T> Default for Outlet<T> {
  fn default() -> Self {
    Self::new()
  }
}
