//! Delivery request for a batch.

use alloc::vec::Vec;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{PubSubBatch, PubSubSubscriber, PubSubTopic, PubSubTopicOptions};

/// Delivery request passed to `DeliveryEndpoint`.
pub struct DeliverBatchRequest<TB: RuntimeToolbox> {
  /// Target topic.
  pub topic:       PubSubTopic,
  /// Batch payload.
  pub batch:       PubSubBatch,
  /// Subscribers to deliver to.
  pub subscribers: Vec<PubSubSubscriber<TB>>,
  /// Effective delivery options.
  pub options:     PubSubTopicOptions,
}
