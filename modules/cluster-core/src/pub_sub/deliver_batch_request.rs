//! Delivery request for a batch.

use alloc::vec::Vec;

use super::{PubSubBatch, PubSubSubscriber, PubSubTopic, PubSubTopicOptions};

/// Delivery request passed to `DeliveryEndpoint`.
pub struct DeliverBatchRequest {
  /// Target topic.
  pub topic:       PubSubTopic,
  /// Batch payload.
  pub batch:       PubSubBatch,
  /// Subscribers to deliver to.
  pub subscribers: Vec<PubSubSubscriber>,
  /// Effective delivery options.
  pub options:     PubSubTopicOptions,
}
