#![allow(dead_code)]

use core::marker::PhantomData;

use super::graph_dsl_builder::GraphDslBuilder;
use crate::{
  StreamError,
  dsl::{Flow, Sink},
  shape::{Inlet, Outlet, PortId},
};

#[cfg(test)]
mod tests;

/// Forward port combinator wrapping an [`Outlet`] for ergonomic chaining.
///
/// Provides [`via`](Self::via) and [`to`](Self::to) methods that internally
/// call [`GraphDslBuilder::wire_via`] and [`GraphDslBuilder::wire_to`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PortOps<T> {
  id:  PortId,
  _pd: PhantomData<fn() -> T>,
}

impl<T> PortOps<T> {
  /// Creates a new `PortOps` from an outlet.
  #[must_use]
  pub(crate) fn new(outlet: &Outlet<T>) -> Self {
    Self { id: outlet.id(), _pd: PhantomData }
  }

  /// Returns the wrapped outlet.
  #[must_use]
  pub(crate) const fn outlet(&self) -> Outlet<T> {
    Outlet::from_id(self.id)
  }
}

impl<T: Send + Sync + 'static> PortOps<T> {
  /// Connects through a flow, returning `PortOps` for the flow's outlet.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the connection fails.
  pub(crate) fn via<U, Mat2, BIn, BOut, BMat>(
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
  pub(crate) fn to<Mat2, BIn, BOut, BMat>(
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
  pub(crate) fn connect_to<BIn, BOut, BMat>(
    self,
    inlet: &Inlet<T>,
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
