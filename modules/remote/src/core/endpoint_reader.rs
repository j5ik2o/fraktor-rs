//! Converts serialized remoting envelopes back into runtime messages.

#[cfg(test)]
mod tests;

use alloc::sync::Arc;

use fraktor_actor_rs::core::{
  actor::{actor_path::ActorPath, actor_ref::ActorRefGeneric},
  dead_letter::DeadLetterReason,
  error::SendError,
  messaging::AnyMessageGeneric,
  serialization::SerializationExtensionSharedGeneric,
  system::{ActorSystemWeakGeneric, RemoteWatchHookShared},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::{ArcShared, SharedAccess},
};

#[cfg(feature = "tokio-transport")]
use crate::core::tokio_actor_ref_provider::TokioActorRefProviderGeneric;
use crate::core::{
  endpoint_reader_error::EndpointReaderError, inbound_envelope::InboundEnvelope,
  remote_actor_ref_provider::RemoteActorRefProviderGeneric, remoting_envelope::RemotingEnvelope,
};

/// Deserializes inbound transport envelopes into runtime messages.
///
/// Uses a weak reference to the actor system to avoid circular references.
pub struct EndpointReaderGeneric<TB: RuntimeToolbox + 'static> {
  system:        ActorSystemWeakGeneric<TB>,
  serialization: SerializationExtensionSharedGeneric<TB>,
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
  ///
  /// The reader stores a weak reference to the actor system.
  #[must_use]
  pub fn new(system: ActorSystemWeakGeneric<TB>, serialization: SerializationExtensionSharedGeneric<TB>) -> Self {
    Self { system, serialization }
  }

  /// Decodes a remoting envelope into an inbound representation.
  pub fn decode(&self, envelope: RemotingEnvelope) -> Result<InboundEnvelope<TB>, EndpointReaderError> {
    let recipient = envelope.recipient().clone();
    let remote_node = envelope.remote_node().clone();
    let sender = envelope.sender().cloned();
    let correlation = envelope.correlation_id();
    let priority = envelope.priority();
    let serialized = envelope.serialized_message().clone();
    match self.deserialize_message(&serialized) {
      | Ok(message) => Ok(InboundEnvelope::new(recipient, remote_node, message, sender, correlation, priority)),
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
    let payload = self.serialization.with_read(|ext| ext.deserialize(serialized, None))?;
    let arc: Arc<dyn core::any::Any + Send + Sync + 'static> = payload.into();
    #[cfg(feature = "force-portable-arc")]
    let shared = ArcShared::___from_arc(arc.into());
    #[cfg(not(feature = "force-portable-arc"))]
    let shared = ArcShared::___from_arc(arc);
    Ok(AnyMessageGeneric::from_erased(shared, None))
  }

  fn record_deserialization_failure(&self, recipient: &ActorPath) {
    if let Some(system) = self.system.upgrade() {
      let message = AnyMessageGeneric::new(recipient.clone());
      system.record_dead_letter(message, DeadLetterReason::SerializationError, None);
    }
  }

  /// Delivers the provided inbound envelope to the actor system.
  ///
  /// Returns an error if the actor system has been dropped or the recipient is unavailable.
  pub fn deliver(&self, inbound: InboundEnvelope<TB>) -> Result<(), SendError<TB>> {
    let Some(system) = self.system.upgrade() else {
      let (_, message, _) = inbound.into_delivery_parts();
      return Err(SendError::closed(message));
    };
    let (recipient, mut message, sender_path) = inbound.into_delivery_parts();
    if let Some(sender_path) = sender_path
      && let Some(sender_ref) = self.resolve_sender_with_system(&system, &sender_path)
    {
      message = message.with_sender(sender_ref);
    }
    if let Some(temp_name) = temp_actor_name(&recipient)
      && let Some(temp_ref) = system.state().temp_actor(temp_name)
    {
      let result = temp_ref.tell(message);
      system.state().unregister_temp_actor(temp_name);
      return result;
    }
    let Some(pid) = system.pid_by_path(&recipient) else {
      return self.record_missing_recipient_with_system(&system, recipient, message);
    };
    let Some(actor_ref) = system.actor_ref_by_pid(pid) else {
      return self.record_missing_recipient_with_system(&system, recipient, message);
    };
    actor_ref.tell(message)
  }

  fn record_missing_recipient_with_system(
    &self,
    system: &fraktor_actor_rs::core::system::ActorSystemGeneric<TB>,
    _recipient: ActorPath,
    message: AnyMessageGeneric<TB>,
  ) -> Result<(), SendError<TB>> {
    system.record_dead_letter(message.clone(), DeadLetterReason::RecipientUnavailable, None);
    Err(SendError::no_recipient(message))
  }

  fn resolve_sender_with_system(
    &self,
    system: &fraktor_actor_rs::core::system::ActorSystemGeneric<TB>,
    path: &ActorPath,
  ) -> Option<ActorRefGeneric<TB>> {
    // Try Tokio provider first when available, then generic remote provider as fallback.
    #[cfg(feature = "tokio-transport")]
    if let Some(provider) =
      system.extended().actor_ref_provider::<RemoteWatchHookShared<TB, TokioActorRefProviderGeneric<TB>>>()
      && let Ok(sender_ref) = provider.get_actor_ref(path.clone())
    {
      return Some(sender_ref);
    }

    if let Some(provider) =
      system.extended().actor_ref_provider::<RemoteWatchHookShared<TB, RemoteActorRefProviderGeneric<TB>>>()
      && let Ok(sender_ref) = provider.get_actor_ref(path.clone())
    {
      return Some(sender_ref);
    }

    None
  }
}

fn temp_actor_name(path: &ActorPath) -> Option<&str> {
  let segments = path.segments();
  if segments.len() < 3 {
    return None;
  }
  if segments[1].as_str() != "temp" {
    return None;
  }
  Some(segments[2].as_str())
}
