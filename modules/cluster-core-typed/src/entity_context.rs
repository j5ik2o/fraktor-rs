//! Typed execution context for sharded entities.

#[cfg(test)]
#[path = "entity_context_test.rs"]
mod tests;

use core::marker::PhantomData;

use fraktor_cluster_core_kernel_rs::{
  extension::ClusterApi,
  grain::{GrainContext, GrainContextImpl},
};

use crate::GrainTypeKey;

/// Typed facade over the kernel grain execution context for message type `M`.
///
/// This is the fraktor equivalent of Pekko's `EntityContext[M]`. It wraps
/// [`GrainContextImpl`](fraktor_cluster_core_kernel_rs::grain::GrainContextImpl)
/// and exposes typed accessors for the entity type key and entity id.
pub struct EntityContext<M> {
  inner:    GrainContextImpl,
  _message: PhantomData<fn() -> M>,
}

impl<M> EntityContext<M> {
  /// Wraps a kernel grain context with an asserted message type `M`.
  #[must_use]
  pub const fn from_kernel(inner: GrainContextImpl) -> Self {
    Self { inner, _message: PhantomData }
  }

  /// Returns a reference to the underlying kernel grain context.
  #[must_use]
  pub const fn as_kernel(&self) -> &GrainContextImpl {
    &self.inner
  }

  /// Consumes this wrapper and returns the underlying kernel grain context.
  #[must_use]
  pub fn into_kernel(self) -> GrainContextImpl {
    self.inner
  }

  /// Returns the grain kind name.
  #[must_use]
  pub fn kind(&self) -> &str {
    self.inner.kind()
  }

  /// Returns the business-domain entity id.
  #[must_use]
  pub fn entity_id(&self) -> &str {
    self.inner.identity()
  }

  /// Returns the typed grain type key derived from the stored kind.
  #[must_use]
  pub fn type_key(&self) -> GrainTypeKey<M> {
    GrainTypeKey::new(self.inner.kind())
  }

  /// Returns the cluster API reference exposed to grain implementations.
  #[must_use]
  pub fn cluster(&self) -> &ClusterApi {
    self.inner.cluster()
  }
}
