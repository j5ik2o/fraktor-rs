//! Publish-time overrides for topic defaults.

use crate::core::{DeliveryPolicy, PartitionBehavior};

/// Per-publish overrides for delivery policies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PublishOptions {
  /// Override for delivery policy.
  pub delivery_policy:    Option<DeliveryPolicy>,
  /// Override for partition behavior.
  pub partition_behavior: Option<PartitionBehavior>,
}

impl PublishOptions {
  /// Creates a new empty set of overrides.
  #[must_use]
  pub const fn new() -> Self {
    Self { delivery_policy: None, partition_behavior: None }
  }
}
