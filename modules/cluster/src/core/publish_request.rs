//! Publish request payload.

use fraktor_actor_rs::core::messaging::AnyMessageGeneric;
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{PubSubTopic, PublishOptions};

/// Request describing a publish operation.
pub struct PublishRequest<TB: RuntimeToolbox> {
  /// Target topic.
  pub topic:   PubSubTopic,
  /// Payload to publish.
  pub payload: AnyMessageGeneric<TB>,
  /// Publish-time overrides.
  pub options: PublishOptions,
}

impl<TB: RuntimeToolbox> PublishRequest<TB> {
  /// Creates a new publish request.
  #[must_use]
  pub const fn new(topic: PubSubTopic, payload: AnyMessageGeneric<TB>, options: PublishOptions) -> Self {
    Self { topic, payload, options }
  }
}
