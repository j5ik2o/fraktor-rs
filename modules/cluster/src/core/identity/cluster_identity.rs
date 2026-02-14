//! Cluster identity model representing kind/identity pairs.

#[cfg(test)]
mod tests;

use alloc::{format, string::String};

use super::cluster_identity_error::ClusterIdentityError;
use crate::core::grain::GrainKey;

/// Identifies a virtual actor using `kind` and `identity`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClusterIdentity {
  kind:     String,
  identity: String,
}

impl ClusterIdentity {
  /// Creates a new cluster identity.
  ///
  /// # Errors
  ///
  /// Returns an error if `kind` or `identity` is empty.
  pub fn new(kind: impl Into<String>, identity: impl Into<String>) -> Result<Self, ClusterIdentityError> {
    let kind = kind.into();
    if kind.is_empty() {
      return Err(ClusterIdentityError::EmptyKind);
    }
    let identity = identity.into();
    if identity.is_empty() {
      return Err(ClusterIdentityError::EmptyIdentity);
    }
    Ok(Self { kind, identity })
  }

  /// Returns the kind component.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn kind(&self) -> &str {
    &self.kind
  }

  /// Returns the identity component.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn identity(&self) -> &str {
    &self.identity
  }

  /// Returns the grain key derived from this identity.
  #[must_use]
  pub fn key(&self) -> GrainKey {
    GrainKey::new(format!("{}/{}", self.kind, self.identity))
  }
}
