#![allow(dead_code)]

use core::marker::PhantomData;

use super::graph_dsl_builder::GraphDslBuilder;
use crate::{
  StreamError,
  dsl::Source,
  shape::{Inlet, Outlet, PortId},
};

#[cfg(test)]
mod tests;

/// Reverse port combinator wrapping an [`Inlet`] for ergonomic chaining.
///
/// Provides [`wire_from_source`](Self::wire_from_source) and
/// [`connect_from`](Self::connect_from) methods that internally call
/// [`GraphDslBuilder::wire_from`] and [`GraphDslBuilder::connect`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ReversePortOps<T> {
  id:  PortId,
  _pd: PhantomData<fn() -> T>,
}

impl<T> ReversePortOps<T> {
  /// Creates a new `ReversePortOps` from an inlet.
  #[must_use]
  pub(crate) fn new(inlet: &Inlet<T>) -> Self {
    Self { id: inlet.id(), _pd: PhantomData }
  }

  /// Returns the wrapped inlet.
  #[must_use]
  pub(crate) const fn inlet(&self) -> Inlet<T> {
    Inlet::from_id(self.id)
  }
}

impl<T: Send + Sync + 'static> ReversePortOps<T> {
  /// Connects a source to this inlet.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the connection fails.
  pub(crate) fn wire_from_source<Mat2, BIn, BOut, BMat>(
    self,
    source: Source<T, Mat2>,
    b: &mut GraphDslBuilder<BIn, BOut, BMat>,
  ) -> Result<(), StreamError> {
    b.wire_from(source, &self.inlet())
  }

  /// Connects from an outlet to this inlet.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the connection fails.
  pub(crate) fn connect_from<BIn, BOut, BMat>(
    self,
    outlet: &Outlet<T>,
    b: &mut GraphDslBuilder<BIn, BOut, BMat>,
  ) -> Result<(), StreamError> {
    b.connect(outlet, &self.inlet())
  }
}

impl<T> From<Inlet<T>> for ReversePortOps<T> {
  fn from(inlet: Inlet<T>) -> Self {
    Self::new(&inlet)
  }
}
