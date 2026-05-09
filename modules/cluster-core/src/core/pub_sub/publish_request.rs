//! Publish request payload.

use fraktor_actor_core_rs::actor::messaging::AnyMessage;

use super::{PubSubTopic, PublishOptions};

/// Request describing a publish operation.
pub struct PublishRequest {
  /// Target topic.
  pub topic:   PubSubTopic,
  /// Payload to publish.
  pub payload: AnyMessage,
  /// Publish-time overrides.
  pub options: PublishOptions,
}

impl PublishRequest {
  /// Creates a new publish request.
  #[must_use]
  pub const fn new(topic: PubSubTopic, payload: AnyMessage, options: PublishOptions) -> Self {
    Self { topic, payload, options }
  }
}
