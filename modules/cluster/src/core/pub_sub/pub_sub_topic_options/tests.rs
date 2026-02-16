use super::PubSubTopicOptions;
use crate::core::pub_sub::{DeliveryPolicy, PartitionBehavior, PublishOptions};

#[test]
fn overrides_apply_on_top_of_defaults() {
  let defaults = PubSubTopicOptions {
    delivery_policy:    DeliveryPolicy::AtLeastOnce,
    partition_behavior: PartitionBehavior::DelayQueue,
  };

  let overrides = PublishOptions { delivery_policy: Some(DeliveryPolicy::AtMostOnce), partition_behavior: None };
  let merged = defaults.apply_overrides(&overrides);

  assert_eq!(merged.delivery_policy, DeliveryPolicy::AtMostOnce);
  assert_eq!(merged.partition_behavior, PartitionBehavior::DelayQueue);
}

#[test]
fn overrides_can_replace_partition_behavior() {
  let defaults = PubSubTopicOptions {
    delivery_policy:    DeliveryPolicy::AtLeastOnce,
    partition_behavior: PartitionBehavior::DelayQueue,
  };

  let overrides = PublishOptions { delivery_policy: None, partition_behavior: Some(PartitionBehavior::Drop) };
  let merged = defaults.apply_overrides(&overrides);

  assert_eq!(merged.delivery_policy, DeliveryPolicy::AtLeastOnce);
  assert_eq!(merged.partition_behavior, PartitionBehavior::Drop);
}
