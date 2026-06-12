//! Typed grain kind declaration point.

#[cfg(test)]
#[path = "grain_type_key_test.rs"]
mod tests;

use alloc::string::String;
use core::marker::PhantomData;

use fraktor_cluster_core_kernel_rs::activation::ClusterIdentityError;

use crate::ClusterIdentity;

/// Declaration point associating a grain kind with a message type.
///
/// This is the fraktor equivalent of Pekko's `EntityTypeKey[M]`. It holds a
/// grain kind string and derives typed cluster identities for individual
/// entity ids. Validation is fully delegated to the kernel
/// [`ClusterIdentity`](fraktor_cluster_core_kernel_rs::activation::ClusterIdentity).
///
/// # Note on `new` being infallible
///
/// The kernel exposes no kind-only validation API; validation requires both
/// `kind` and `entity_id`. Therefore `new` accepts any `&str` and stores it.
/// Invalidity (e.g. empty kind) is surfaced when `identity_for` is called and
/// the kernel rejects the combination.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GrainTypeKey<M> {
  kind:     String,
  _message: PhantomData<fn() -> M>,
}

impl<M> GrainTypeKey<M> {
  /// Creates a grain type key for the given kind.
  ///
  /// This constructor is infallible because the kernel provides no kind-only
  /// validation. Validation is deferred to [`identity_for`](Self::identity_for).
  #[must_use]
  pub fn new(kind: &str) -> Self {
    Self { kind: kind.into(), _message: PhantomData }
  }

  /// Returns the grain kind name.
  #[must_use]
  pub fn kind(&self) -> &str {
    &self.kind
  }

  /// Derives a typed cluster identity for the given entity id.
  ///
  /// Delegates validation of both kind and entity id to the kernel
  /// [`ClusterIdentity::new`](fraktor_cluster_core_kernel_rs::activation::ClusterIdentity::new).
  ///
  /// # Errors
  ///
  /// Returns [`ClusterIdentityError::EmptyKind`] when the stored kind is empty,
  /// or [`ClusterIdentityError::EmptyIdentity`] when `entity_id` is empty.
  pub fn identity_for(&self, entity_id: &str) -> Result<ClusterIdentity<M>, ClusterIdentityError> {
    ClusterIdentity::new(self.kind.as_str(), entity_id)
  }
}
