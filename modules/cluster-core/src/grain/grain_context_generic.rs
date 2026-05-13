//! Default grain execution context.

#[cfg(test)]
#[path = "grain_context_generic_test.rs"]
mod tests;

use super::GrainContext;
use crate::{ClusterApi, identity::ClusterIdentity};

/// Default grain execution context.
pub struct GrainContextImpl {
  identity: ClusterIdentity,
  cluster:  ClusterApi,
}

impl GrainContextImpl {
  /// Creates a new grain context.
  #[must_use]
  pub const fn new(identity: ClusterIdentity, cluster: ClusterApi) -> Self {
    Self { identity, cluster }
  }
}

impl GrainContext for GrainContextImpl {
  fn kind(&self) -> &str {
    self.identity.kind()
  }

  fn identity(&self) -> &str {
    self.identity.identity()
  }

  fn cluster(&self) -> &ClusterApi {
    &self.cluster
  }
}
