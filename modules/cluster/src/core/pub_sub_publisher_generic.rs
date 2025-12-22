//! Publisher that serializes messages for pub/sub delivery.

#[cfg(test)]
mod tests;

use alloc::{format, string::String, vec, vec::Vec};
use core::any::type_name_of_val;

use fraktor_actor_rs::core::{
  messaging::AnyMessageGeneric,
  serialization::{SerializationError, SerializationRegistryGeneric},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeToolbox,
  sync::{ArcShared, SharedAccess},
};

use crate::core::{
  ClusterPubSubShared, PubSubBatch, PubSubEnvelope, PubSubError, PubSubTopic, PublishAck, PublishOptions,
  PublishRejectReason, PublishRequest,
};

/// Publishes messages by serializing them into pub/sub batches.
pub struct PubSubPublisherGeneric<TB: RuntimeToolbox + 'static> {
  pub_sub:  ClusterPubSubShared<TB>,
  registry: ArcShared<SerializationRegistryGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for PubSubPublisherGeneric<TB> {
  fn clone(&self) -> Self {
    Self { pub_sub: self.pub_sub.clone(), registry: self.registry.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> PubSubPublisherGeneric<TB> {
  /// Creates a new publisher.
  #[must_use]
  pub const fn new(pub_sub: ClusterPubSubShared<TB>, registry: ArcShared<SerializationRegistryGeneric<TB>>) -> Self {
    Self { pub_sub, registry }
  }

  /// Publishes a single payload.
  ///
  /// # Errors
  ///
  /// Returns `PubSubError` for system-level failures.
  pub fn publish(&self, request: &PublishRequest<TB>) -> Result<PublishAck, PubSubError> {
    let topic = request.topic.clone();
    if topic.is_empty() {
      return Ok(PublishAck::rejected(PublishRejectReason::InvalidTopic));
    }

    if let Some(batch) = request.payload.payload().downcast_ref::<PubSubBatch>() {
      return self.publish_batch(&topic, batch.clone(), request.options);
    }

    let envelope = match self.serialize_message(&request.payload) {
      | Ok(envelope) => envelope,
      | Err(error) => return Self::map_serialization_error(&error),
    };

    let batch = PubSubBatch::new(vec![envelope]);
    self.publish_batch(&topic, batch, request.options)
  }

  /// Publishes a pre-built batch.
  ///
  /// # Errors
  ///
  /// Returns `PubSubError` for system-level failures.
  pub fn publish_batch(
    &self,
    topic: &PubSubTopic,
    batch: PubSubBatch,
    options: PublishOptions,
  ) -> Result<PublishAck, PubSubError> {
    let request = PublishRequest::new(topic.clone(), AnyMessageGeneric::new(batch), options);
    self.pub_sub.with_write(|pub_sub| pub_sub.publish(request))
  }

  pub(crate) fn build_batch(&self, messages: Vec<AnyMessageGeneric<TB>>) -> Result<PubSubBatch, PublishRejectReason> {
    let mut envelopes = Vec::with_capacity(messages.len());
    for message in messages {
      let envelope = match self.serialize_message(&message) {
        | Ok(envelope) => envelope,
        | Err(error) => return Err(map_reject_reason(&error)),
      };
      envelopes.push(envelope);
    }
    Ok(PubSubBatch::new(envelopes))
  }

  fn serialize_message(&self, message: &AnyMessageGeneric<TB>) -> Result<PubSubEnvelope, SerializationError> {
    let payload = message.payload();
    let type_id = payload.type_id();
    let type_name = self.registry.binding_name(type_id).unwrap_or_else(|| String::from(type_name_of_val(payload)));

    let (serializer, _) = self.registry.serializer_for_type(type_id, &type_name, None)?;
    let bytes = serializer.to_binary(payload)?;
    let mut name = type_name;
    if let Some(provider) = serializer.as_string_manifest() {
      name = provider.manifest(payload).into_owned();
    }
    Ok(PubSubEnvelope { serializer_id: serializer.identifier().value(), type_name: name, bytes })
  }

  fn map_serialization_error(error: &SerializationError) -> Result<PublishAck, PubSubError> {
    if error.is_not_serializable() {
      return Ok(PublishAck::rejected(PublishRejectReason::NotSerializable));
    }
    Err(PubSubError::SerializationFailed { reason: format!("{error:?}") })
  }
}

const fn map_reject_reason(error: &SerializationError) -> PublishRejectReason {
  if error.is_not_serializable() { PublishRejectReason::NotSerializable } else { PublishRejectReason::InvalidPayload }
}
