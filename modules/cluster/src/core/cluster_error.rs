//! Consolidated cluster errors.

use crate::core::{ClusterProviderError, IdentitySetupError};

/// Error type returned by cluster lifecycle operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClusterError {
  /// Provider-related failure.
  Provider(ClusterProviderError),
  /// Identity lookup setup failure.
  Identity(IdentitySetupError),
}

impl From<ClusterProviderError> for ClusterError {
  fn from(value: ClusterProviderError) -> Self {
    Self::Provider(value)
  }
}

impl From<IdentitySetupError> for ClusterError {
  fn from(value: IdentitySetupError) -> Self {
    Self::Identity(value)
  }
}
