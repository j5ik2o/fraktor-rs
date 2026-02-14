//! Default delivery options per topic.

#[cfg(test)]
mod tests;

use super::{DeliveryPolicy, PartitionBehavior, PublishOptions};

/// Topic-level default policies for pub/sub delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PubSubTopicOptions {
  /// Delivery policy for the topic.
  pub delivery_policy:    DeliveryPolicy,
  /// Partition handling behavior.
  pub partition_behavior: PartitionBehavior,
}

impl PubSubTopicOptions {
  /// Returns the system default options.
  #[must_use]
  pub const fn system_default() -> Self {
    Self { delivery_policy: DeliveryPolicy::AtLeastOnce, partition_behavior: PartitionBehavior::DelayQueue }
  }

  /// Applies publish-time overrides on top of the current defaults.
  #[must_use]
  pub fn apply_overrides(&self, overrides: &PublishOptions) -> Self {
    Self {
      delivery_policy:    overrides.delivery_policy.unwrap_or(self.delivery_policy),
      partition_behavior: overrides.partition_behavior.unwrap_or(self.partition_behavior),
    }
  }
}

impl Default for PubSubTopicOptions {
  fn default() -> Self {
    Self::system_default()
  }
}
