//! Default grain execution context.

#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::GrainContext;
use crate::core::{ClusterApiGeneric, identity::ClusterIdentity};

/// Default grain execution context.
pub struct GrainContextGeneric<TB: RuntimeToolbox + 'static> {
  identity: ClusterIdentity,
  cluster:  ClusterApiGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> GrainContextGeneric<TB> {
  /// Creates a new grain context.
  #[must_use]
  pub const fn new(identity: ClusterIdentity, cluster: ClusterApiGeneric<TB>) -> Self {
    Self { identity, cluster }
  }
}

impl<TB: RuntimeToolbox + 'static> GrainContext<TB> for GrainContextGeneric<TB> {
  fn kind(&self) -> &str {
    self.identity.kind()
  }

  fn identity(&self) -> &str {
    self.identity.identity()
  }

  fn cluster(&self) -> &ClusterApiGeneric<TB> {
    &self.cluster
  }
}
