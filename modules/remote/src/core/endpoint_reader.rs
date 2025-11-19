//! Converts serialized remoting envelopes back into runtime messages.

#[cfg(test)]
mod tests;

use alloc::sync::Arc;

use fraktor_actor_rs::core::{
  actor_prim::actor_path::ActorPath, dead_letter::DeadLetterReason, error::SendError, messaging::AnyMessageGeneric,
  serialization::SerializationExtensionGeneric, system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use crate::core::{
  endpoint_reader_error::EndpointReaderError, inbound_envelope::InboundEnvelope, remoting_envelope::RemotingEnvelope,
};

/// Deserializes inbound transport envelopes into runtime messages.
pub struct EndpointReaderGeneric<TB: RuntimeToolbox + 'static> {
  system:        ActorSystemGeneric<TB>,
  serialization: ArcShared<SerializationExtensionGeneric<TB>>,
}

/// Type alias for `EndpointReaderGeneric` with the default `NoStdToolbox`.
pub type EndpointReader = EndpointReaderGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> Clone for EndpointReaderGeneric<TB> {
  fn clone(&self) -> Self {
    Self { system: self.system.clone(), serialization: self.serialization.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> EndpointReaderGeneric<TB> {
  /// Creates a new reader bound to the provided actor system.
  #[must_use]
  pub fn new(system: ActorSystemGeneric<TB>, serialization: ArcShared<SerializationExtensionGeneric<TB>>) -> Self {
    Self { system, serialization }
  }

  /// Decodes a remoting envelope into an inbound representation.
  pub fn decode(&self, envelope: RemotingEnvelope) -> Result<InboundEnvelope<TB>, EndpointReaderError> {
    let recipient = envelope.recipient().clone();
    let remote_node = envelope.remote_node().clone();
    let reply_to = envelope.reply_to().cloned();
    let correlation = envelope.correlation_id();
    let priority = envelope.priority();
    let serialized = envelope.serialized_message().clone();
    match self.deserialize_message(&serialized) {
      | Ok(message) => Ok(InboundEnvelope::new(recipient, remote_node, message, reply_to, correlation, priority)),
      | Err(error) => {
        self.record_deserialization_failure(&recipient);
        Err(EndpointReaderError::Deserialization(error))
      },
    }
  }

  fn deserialize_message(
    &self,
    serialized: &fraktor_actor_rs::core::serialization::SerializedMessage,
  ) -> Result<AnyMessageGeneric<TB>, fraktor_actor_rs::core::serialization::SerializationError> {
    let payload = self.serialization.deserialize(serialized, None)?;
    let arc: Arc<dyn core::any::Any + Send + Sync + 'static> = payload.into();
    let shared = ArcShared::from_arc(arc);
    Ok(AnyMessageGeneric::from_erased(shared, None))
  }

  fn record_deserialization_failure(&self, recipient: &ActorPath) {
    let message = AnyMessageGeneric::new(recipient.clone());
    self.system.record_dead_letter(message, DeadLetterReason::SerializationError, None);
  }

  /// Delivers the provided inbound envelope to the actor system.
  pub fn deliver(&self, inbound: InboundEnvelope<TB>) -> Result<(), SendError<TB>> {
    let (recipient, message, _reply_to) = inbound.into_delivery_parts();
    let Some(pid) = self.system.pid_by_path(&recipient) else {
      return self.record_missing_recipient(recipient, message);
    };
    let Some(actor_ref) = self.system.actor_ref_by_pid(pid) else {
      return self.record_missing_recipient(recipient, message);
    };
    actor_ref.tell(message)
  }

  fn record_missing_recipient(
    &self,
    _recipient: ActorPath,
    message: AnyMessageGeneric<TB>,
  ) -> Result<(), SendError<TB>> {
    self.system.record_dead_letter(message.clone(), DeadLetterReason::RecipientUnavailable, None);
    Err(SendError::no_recipient(message))
  }
}
