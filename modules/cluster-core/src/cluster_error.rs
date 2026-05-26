//! Consolidated cluster errors.

extern crate alloc;

use alloc::string::String;

use crate::{
  ClusterProviderError, downing_provider::DowningDecision, identity::IdentitySetupError, pub_sub::PubSubError,
};

/// Error type returned by cluster lifecycle operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClusterError {
  /// Provider-related failure.
  Provider(ClusterProviderError),
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
