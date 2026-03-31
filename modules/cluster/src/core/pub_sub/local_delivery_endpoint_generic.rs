//! no_std local delivery endpoint implementation.

use alloc::{format, vec::Vec};
use core::any::TypeId;

use fraktor_actor_rs::core::kernel::{
  actor::messaging::AnyMessage,
  serialization::{SerializationError, SerializerId, serialization_registry::SerializationRegistry},
};
use fraktor_utils_rs::core::sync::ArcShared;

use super::{
  DeliverBatchRequest, DeliveryEndpoint, DeliveryReport, DeliveryStatus, PubSubAutoRespondBatch, PubSubBatch,
  PubSubError, PubSubSubscriber, SubscriberDeliveryReport,
};

/// Local-only delivery endpoint for no_std usage.
pub struct LocalDeliveryEndpoint {
  registry: ArcShared<SerializationRegistry>,
}

impl LocalDeliveryEndpoint {
  /// Creates a new local delivery endpoint.
  #[must_use]
  pub const fn new(registry: ArcShared<SerializationRegistry>) -> Self {
    Self { registry }
  }
}

impl DeliveryEndpoint for LocalDeliveryEndpoint {
  fn deliver(&mut self, request: DeliverBatchRequest) -> Result<DeliveryReport, PubSubError> {
    let messages = deserialize_batch(&self.registry, &request.batch)
      .map_err(|error| PubSubError::SerializationFailed { reason: format!("{error:?}") })?;
    let payload = AnyMessage::new(PubSubAutoRespondBatch { messages });

    let mut failed = Vec::new();
    for subscriber in request.subscribers {
      match subscriber {
        | PubSubSubscriber::ActorRef(mut actor_ref) => {
          if actor_ref.try_tell(payload.clone()).is_err() {
            failed.push(SubscriberDeliveryReport {
              subscriber: PubSubSubscriber::ActorRef(actor_ref),
              status:     DeliveryStatus::SubscriberUnreachable,
            });
          }
        },
        | PubSubSubscriber::ClusterIdentity(identity) => {
          failed.push(SubscriberDeliveryReport {
            subscriber: PubSubSubscriber::ClusterIdentity(identity),
            status:     DeliveryStatus::SubscriberUnreachable,
          });
        },
      }
    }

    Ok(DeliveryReport { status: aggregate_status(&failed), failed })
  }
}

fn deserialize_batch(
  registry: &ArcShared<SerializationRegistry>,
  batch: &PubSubBatch,
) -> Result<Vec<AnyMessage>, SerializationError> {
  let mut messages = Vec::with_capacity(batch.envelopes.len());
  for envelope in &batch.envelopes {
    let serializer_id =
      SerializerId::try_from(envelope.serializer_id).map_err(|_| SerializationError::invalid_format())?;
    let serializer = registry.serializer_by_id(serializer_id)?;
    let value = if let Some(provider) = serializer.as_string_manifest() {
      provider.from_binary_with_manifest(&envelope.bytes, &envelope.type_name)?
    } else {
      serializer.from_binary(&envelope.bytes, None::<TypeId>)?
    };
    messages.push(AnyMessage::new(value));
  }
  Ok(messages)
}

fn aggregate_status(failed: &[SubscriberDeliveryReport]) -> DeliveryStatus {
  if failed.is_empty() {
    return DeliveryStatus::Delivered;
  }
  if failed.iter().any(|report| matches!(report.status, DeliveryStatus::Timeout)) {
    return DeliveryStatus::Timeout;
  }
  if failed.iter().any(|report| matches!(report.status, DeliveryStatus::SubscriberUnreachable)) {
    return DeliveryStatus::SubscriberUnreachable;
  }
  DeliveryStatus::OtherError
}
