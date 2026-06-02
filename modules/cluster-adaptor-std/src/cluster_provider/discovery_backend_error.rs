//! Observable generic discovery backend failure.

use std::{
  error::Error,
  fmt::{Display, Formatter, Result},
};

use fraktor_cluster_core_kernel_rs::extension::ClusterProviderError;

/// Error reported by a generic discovery backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryBackendError {
  /// Backend failed temporarily and should be observed without clearing topology.
  Temporary(String),
}

impl DiscoveryBackendError {
  /// Creates a temporary backend failure.
  #[must_use]
  pub fn temporary(reason: impl Into<String>) -> Self {
    Self::Temporary(reason.into())
  }

  /// Returns the failure reason.
  #[must_use]
  pub const fn reason(&self) -> &str {
    match self {
      | Self::Temporary(reason) => reason.as_str(),
    }
  }
}

impl Display for DiscoveryBackendError {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
    formatter.write_str(self.reason())
  }
}

impl Error for DiscoveryBackendError {}

impl From<DiscoveryBackendError> for ClusterProviderError {
  fn from(error: DiscoveryBackendError) -> Self {
    ClusterProviderError::start_member(error.reason())
  }
}
