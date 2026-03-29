use core::marker::PhantomData;

use super::{GraphDslBuilder, StreamError, shape::Outlet};
use crate::core::{
  dsl::{Flow, Sink},
  shape::PortId,
};

#[cfg(test)]
mod tests;

/// Forward port combinator wrapping an [`Outlet`] for ergonomic chaining.
///
/// Provides [`via`](Self::via) and [`to`](Self::to) methods that internally
/// call [`GraphDslBuilder::wire_via`] and [`GraphDslBuilder::wire_to`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortOps<T> {
  id:  PortId,
  _pd: PhantomData<fn() -> T>,
}

impl<T> PortOps<T> {
  /// Creates a new `PortOps` from an outlet.
  #[must_use]
  pub fn new(outlet: &Outlet<T>) -> Self {
    Self { id: outlet.id(), _pd: PhantomData }
  }

  /// Returns the wrapped outlet.
  #[must_use]
  pub const fn outlet(&self) -> Outlet<T> {
    Outlet::from_id(self.id)
  }
}

impl<T: Send + Sync + 'static> PortOps<T> {
  /// Connects through a flow, returning `PortOps` for the flow's outlet.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the connection fails.
  pub fn via<U, Mat2, BIn, BOut, BMat>(
    self,
    flow: Flow<T, U, Mat2>,
    b: &mut GraphDslBuilder<BIn, BOut, BMat>,
  ) -> Result<PortOps<U>, StreamError>
  where
    U: Send + Sync + 'static, {
    b.wire_via(&self.outlet(), flow).map(|o| PortOps::new(&o))
  }

  /// Connects to a sink (terminal).
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the connection fails.
  pub fn to<Mat2, BIn, BOut, BMat>(
    self,
    sink: Sink<T, Mat2>,
    b: &mut GraphDslBuilder<BIn, BOut, BMat>,
  ) -> Result<(), StreamError> {
    b.wire_to(&self.outlet(), sink)
  }

  /// Connects to an inlet (terminal).
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the connection fails.
  pub fn connect_to<BIn, BOut, BMat>(
    self,
    inlet: &super::shape::Inlet<T>,
    b: &mut GraphDslBuilder<BIn, BOut, BMat>,
  ) -> Result<(), StreamError> {
    b.connect(&self.outlet(), inlet)
  }
}

impl<T> From<Outlet<T>> for PortOps<T> {
  fn from(outlet: Outlet<T>) -> Self {
    Self::new(&outlet)
  }
}
