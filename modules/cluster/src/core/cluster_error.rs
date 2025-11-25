//! Consolidated cluster errors.

use crate::core::{ClusterProviderError, IdentitySetupError, pub_sub_error::PubSubError};

/// Error type returned by cluster lifecycle operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClusterError {
  /// Provider-related failure.
  Provider(ClusterProviderError),
  /// Identity lookup setup failure.
  Identity(IdentitySetupError),
  /// Gossip start/stop failure.
  Gossip(&'static str),
  /// PubSub start/stop failure.
  PubSub(PubSubError),
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

impl From<PubSubError> for ClusterError {
  fn from(value: PubSubError) -> Self {
    Self::PubSub(value)
  }
}
