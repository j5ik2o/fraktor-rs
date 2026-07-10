//! Consolidated cluster errors.

extern crate alloc;

#[cfg(test)]
#[path = "cluster_error_test.rs"]
mod tests;

use alloc::string::String;

use crate::{
  ClusterExtensionConfigError, ClusterProviderError, activation::IdentitySetupError, downing_provider::DowningDecision,
  failure_detector::FailureDetectorConfigError, pub_sub::PubSubError,
};

/// Error type returned by cluster lifecycle operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClusterError {
  /// Provider-related failure.
  Provider(ClusterProviderError),
  /// Cluster configuration validation failure.
  Configuration(ClusterExtensionConfigError),
  /// Downing strategy did not allow an explicit down command.
  DowningRejected {
    /// Authority that was requested to be downed.
    authority: String,
    /// Decision returned by the downing strategy.
    decision:  DowningDecision,
  },
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

impl From<FailureDetectorConfigError> for ClusterError {
  fn from(value: FailureDetectorConfigError) -> Self {
    Self::Configuration(value.into())
  }
}

impl From<ClusterExtensionConfigError> for ClusterError {
  fn from(value: ClusterExtensionConfigError) -> Self {
    Self::Configuration(value)
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
