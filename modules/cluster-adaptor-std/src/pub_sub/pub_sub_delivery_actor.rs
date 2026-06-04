//! std-only delivery endpoint implementation.

#[cfg(test)]
#[path = "pub_sub_delivery_actor_test.rs"]
mod tests;

use alloc::{format, vec::Vec};
use core::any::TypeId;

use fraktor_actor_core_kernel_rs::{
  actor::messaging::AnyMessage,
  serialization::{SerializationError, SerializerId, serialization_registry::SerializationRegistry},
};
use fraktor_cluster_core_kernel_rs::{
  extension::ClusterIdentityResolver,
  pub_sub::{
    DeliverBatchRequest, DeliveryEndpoint, DeliveryReport, DeliveryStatus, PubSubAutoRespondBatch, PubSubBatch,
    PubSubConfig, PubSubError, PubSubSubscriber, SubscriberDeliveryReport,
  },
};
use fraktor_utils_core_rs::sync::ArcShared;

/// Delivery endpoint that resolves cluster identities and sends batches.
pub struct PubSubDeliveryActor {
  identity_resolver: Box<dyn ClusterIdentityResolver>,
  registry:          ArcShared<SerializationRegistry>,
  _config:           PubSubConfig,
}

impl PubSubDeliveryActor {
  /// Creates a new delivery actor.
  #[must_use]
  pub fn new(
    identity_resolver: Box<dyn ClusterIdentityResolver>,
    registry: ArcShared<SerializationRegistry>,
    config: PubSubConfig,
  ) -> Self {
    Self { identity_resolver, registry, _config: config }
  }
}

impl DeliveryEndpoint for PubSubDeliveryActor {
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
        | PubSubSubscriber::ClusterIdentity(identity) => match self.identity_resolver.resolve(&identity) {
          | Ok(mut actor_ref) => {
            if actor_ref.try_tell(payload.clone()).is_err() {
              failed.push(SubscriberDeliveryReport {
                subscriber: PubSubSubscriber::ClusterIdentity(identity),
                status:     DeliveryStatus::SubscriberUnreachable,
              });
            }
          },
          | Err(_) => failed.push(SubscriberDeliveryReport {
            subscriber: PubSubSubscriber::ClusterIdentity(identity),
            status:     DeliveryStatus::SubscriberUnreachable,
          }),
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
    let serializer_id = SerializerId::from_raw(envelope.serializer_id);
    let serializer = registry.serializer_by_id(serializer_id)?;
    let value = if let Some(provider) = serializer.as_string_manifest() {
      provider.from_binary_with_manifest(&envelope.bytes, &envelope.type_name)?
    } else {
      serializer.from_binary(&envelope.bytes, None::<TypeId>)?
    };
    messages.push(AnyMessage::from_erased(ArcShared::from_boxed(value), None, false, false));
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
