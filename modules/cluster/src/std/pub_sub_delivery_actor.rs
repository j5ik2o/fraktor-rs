//! std-only delivery endpoint implementation.

use alloc::{format, vec::Vec};
use core::any::TypeId;

use fraktor_actor_rs::core::{
  actor_prim::actor_ref::ActorRefGeneric,
  messaging::AnyMessageGeneric,
  serialization::{SerializationError, SerializationRegistryGeneric, SerializerId},
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  ClusterIdentity, DeliverBatchRequest, DeliveryEndpoint, DeliveryReport, DeliveryStatus, PubSubAutoRespondBatch,
  PubSubConfig, PubSubError, PubSubSubscriber, SubscriberDeliveryReport,
};

/// Resolves cluster identities to actor references.
pub trait ClusterIdentityResolver<TB: RuntimeToolbox + 'static>: Send + Sync {
  /// Returns the actor ref for the identity, or `None` if unavailable.
  fn resolve(&self, identity: &ClusterIdentity) -> Option<ActorRefGeneric<TB>>;
}

/// Delivery endpoint that resolves cluster identities and sends batches.
pub struct PubSubDeliveryActor<TB: RuntimeToolbox + 'static> {
  resolver: Box<dyn ClusterIdentityResolver<TB>>,
  registry: ArcShared<SerializationRegistryGeneric<TB>>,
  _config:  PubSubConfig,
}

impl<TB: RuntimeToolbox + 'static> PubSubDeliveryActor<TB> {
  /// Creates a new delivery actor.
  #[must_use]
  pub const fn new(
    resolver: Box<dyn ClusterIdentityResolver<TB>>,
    registry: ArcShared<SerializationRegistryGeneric<TB>>,
    config: PubSubConfig,
  ) -> Self {
    Self { resolver, registry, _config: config }
  }
}

impl<TB: RuntimeToolbox + 'static> DeliveryEndpoint<TB> for PubSubDeliveryActor<TB> {
  fn deliver(&mut self, request: DeliverBatchRequest<TB>) -> Result<DeliveryReport<TB>, PubSubError> {
    let messages = deserialize_batch(&self.registry, &request.batch)
      .map_err(|error| PubSubError::SerializationFailed { reason: format!("{error:?}") })?;
    let payload = AnyMessageGeneric::new(PubSubAutoRespondBatch { messages });

    let mut failed = Vec::new();
    for subscriber in request.subscribers {
      match subscriber {
        | PubSubSubscriber::ActorRef(actor_ref) => {
          if let Err(error) = actor_ref.tell(payload.clone()) {
            let status = map_send_error(&error);
            failed.push(SubscriberDeliveryReport { subscriber: PubSubSubscriber::ActorRef(actor_ref), status });
          }
        },
        | PubSubSubscriber::ClusterIdentity(identity) => {
          if let Some(actor_ref) = self.resolver.resolve(&identity) {
            if let Err(error) = actor_ref.tell(payload.clone()) {
              let status = map_send_error(&error);
              failed.push(SubscriberDeliveryReport { subscriber: PubSubSubscriber::ClusterIdentity(identity), status });
            }
          } else {
            failed.push(SubscriberDeliveryReport {
              subscriber: PubSubSubscriber::ClusterIdentity(identity),
              status:     DeliveryStatus::SubscriberUnreachable,
            });
          }
        },
      }
    }

    Ok(DeliveryReport { status: aggregate_status(&failed), failed })
  }
}

fn deserialize_batch<TB: RuntimeToolbox>(
  registry: &ArcShared<SerializationRegistryGeneric<TB>>,
  batch: &crate::core::PubSubBatch,
) -> Result<Vec<AnyMessageGeneric<TB>>, SerializationError> {
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
    messages.push(AnyMessageGeneric::new(value));
  }
  Ok(messages)
}

const fn map_send_error<TB: RuntimeToolbox>(error: &fraktor_actor_rs::core::error::SendError<TB>) -> DeliveryStatus {
  use fraktor_actor_rs::core::error::SendError;
  match error {
    | SendError::Timeout(_) => DeliveryStatus::Timeout,
    | _ => DeliveryStatus::SubscriberUnreachable,
  }
}

fn aggregate_status<TB: RuntimeToolbox>(failed: &[SubscriberDeliveryReport<TB>]) -> DeliveryStatus {
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
