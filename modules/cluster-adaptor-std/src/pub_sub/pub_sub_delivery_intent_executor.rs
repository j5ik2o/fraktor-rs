//! Executes core mediator delivery intents through std delivery endpoints.

#[cfg(test)]
#[path = "pub_sub_delivery_intent_executor_test.rs"]
mod tests;

use alloc::{format, string::String, vec};

use fraktor_cluster_core_kernel_rs::pub_sub::{
  DeliverBatchRequest, DeliveryEndpoint, DeliveryReport, DeliveryStatus, MediatorDeliveryIntent, MediatorDeliveryMode,
  PubSubBatch, PubSubError, PubSubTopic, PubSubTopicOptions,
};

/// Executes mediator delivery intents without recalculating target selection.
pub trait PubSubDeliveryIntentExecutor {
  /// Executes a delivery intent through the underlying endpoint.
  ///
  /// # Errors
  ///
  /// Returns endpoint delivery errors.
  fn execute_intent(
    &mut self,
    topic: PubSubTopic,
    intent: MediatorDeliveryIntent,
    options: PubSubTopicOptions,
  ) -> Result<DeliveryReport, PubSubError>;
}

impl<T> PubSubDeliveryIntentExecutor for T
where
  T: DeliveryEndpoint + ?Sized,
{
  fn execute_intent(
    &mut self,
    topic: PubSubTopic,
    intent: MediatorDeliveryIntent,
    options: PubSubTopicOptions,
  ) -> Result<DeliveryReport, PubSubError> {
    match intent {
      | MediatorDeliveryIntent::Deliver { mode: MediatorDeliveryMode::Publish, targets, payload } => self
        .deliver(DeliverBatchRequest { topic, batch: PubSubBatch::new(vec![payload]), subscribers: targets, options }),
      | MediatorDeliveryIntent::Deliver { mode, .. } => Err(PubSubError::DeliveryFailed {
        reason: format!("topic-scoped delivery executor cannot execute {mode:?} path intent"),
      }),
      | MediatorDeliveryIntent::Dropped { .. } | MediatorDeliveryIntent::DroppedTopic { .. } => {
        Ok(DeliveryReport { status: DeliveryStatus::Delivered, failed: vec![] })
      },
      | MediatorDeliveryIntent::DeadLetter { .. } | MediatorDeliveryIntent::DeadLetterTopic { .. } => {
        Err(PubSubError::DeliveryFailed { reason: String::from("dead-letter delivery endpoint is not configured") })
      },
    }
  }
}
