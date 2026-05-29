//! Typed cluster identity wrapper.

#[cfg(test)]
#[path = "cluster_identity_test.rs"]
mod tests;

use alloc::string::String;
use core::marker::PhantomData;

use fraktor_cluster_core_kernel_rs::activation::{ClusterIdentity as KernelClusterIdentity, ClusterIdentityError};

/// Typed wrapper around the kernel cluster identity for message type `M`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClusterIdentity<M> {
  inner:    KernelClusterIdentity,
  _message: PhantomData<fn() -> M>,
}

impl<M> ClusterIdentity<M> {
  /// Creates a typed cluster identity.
  ///
  /// # Errors
  ///
  /// Returns an error when the kernel identity rejects the kind or identity.
  pub fn new(kind: impl Into<String>, identity: impl Into<String>) -> Result<Self, ClusterIdentityError> {
    KernelClusterIdentity::new(kind, identity).map(Self::from_kernel)
  }

  /// Creates a typed wrapper from a kernel identity.
  #[must_use]
  pub const fn from_kernel(inner: KernelClusterIdentity) -> Self {
    Self { inner, _message: PhantomData }
  }

  /// Returns the kind component.
  #[must_use]
  pub fn kind(&self) -> &str {
    self.inner.kind()
  }

  /// Returns the identity component.
  #[must_use]
  pub fn identity(&self) -> &str {
    self.inner.identity()
  }

  /// Returns the wrapped kernel identity.
  #[must_use]
  pub const fn as_kernel(&self) -> &KernelClusterIdentity {
    &self.inner
  }

  /// Converts this wrapper into the kernel identity.
  #[must_use]
  pub fn into_kernel(self) -> KernelClusterIdentity {
    self.inner
  }
}

impl<M> From<KernelClusterIdentity> for ClusterIdentity<M> {
  fn from(inner: KernelClusterIdentity) -> Self {
    Self::from_kernel(inner)
  }
}

impl<M> From<ClusterIdentity<M>> for KernelClusterIdentity {
  fn from(identity: ClusterIdentity<M>) -> Self {
    identity.into_kernel()
  }
}
