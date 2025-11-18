//! Deserializes remoting frames back into runtime messages.

use alloc::sync::Arc;

use fraktor_actor_rs::core::{
  messaging::AnyMessageGeneric,
  serialization::{SerializationError, SerializationExtensionGeneric},
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{endpoint_writer::RemotingEnvelope, inbound_envelope::InboundEnvelope};

#[cfg(test)]
mod tests;

/// Converts transport frames into inbound envelopes.
pub struct EndpointReader<TB: RuntimeToolbox + 'static> {
  serialization: ArcShared<SerializationExtensionGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> EndpointReader<TB> {
  /// Creates a reader from the provided serialization extension.
  #[must_use]
  pub fn new(serialization: ArcShared<SerializationExtensionGeneric<TB>>) -> Self {
    Self { serialization }
  }

  /// Decodes the provided frame bytes into an inbound envelope.
  pub fn read(&self, frame: &[u8]) -> Result<InboundEnvelope<TB>, SerializationError> {
    let envelope = RemotingEnvelope::decode(frame)?;
    let payload_box = self.serialization.deserialize(envelope.payload(), None)?;
    let payload_arc: Arc<dyn core::any::Any + Send + Sync + 'static> = payload_box.into();
    let shared = ArcShared::from_arc(payload_arc);
    let message = AnyMessageGeneric::from_erased(shared, None);
    Ok(InboundEnvelope::new(
      envelope.target().clone(),
      envelope.remote().clone(),
      message,
      envelope.reply_to().cloned(),
    ))
  }
}
