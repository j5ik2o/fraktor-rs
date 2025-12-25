//! Serialization-backed grain codec.

#[cfg(test)]
mod tests;

use alloc::{format, string::String};

use fraktor_actor_rs::core::{
  messaging::AnyMessageGeneric,
  serialization::{SerializationCallScope, SerializationError, SerializationExtensionSharedGeneric},
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::SharedAccess};

use crate::core::{GrainCodec, GrainCodecError};

/// Grain codec backed by the serialization extension.
pub struct SerializationGrainCodec<TB: RuntimeToolbox + 'static> {
  extension: SerializationExtensionSharedGeneric<TB>,
  scope:     SerializationCallScope,
}

impl<TB: RuntimeToolbox + 'static> SerializationGrainCodec<TB> {
  /// Creates a new codec from a serialization extension handle.
  #[must_use]
  pub const fn new(extension: SerializationExtensionSharedGeneric<TB>, scope: SerializationCallScope) -> Self {
    Self { extension, scope }
  }

  /// Retrieves the serialization extension from the actor system.
  ///
  /// # Errors
  ///
  /// Returns [`GrainCodecError::ExtensionUnavailable`] if the extension is not installed.
  pub fn try_from_system(
    system: &ActorSystemGeneric<TB>,
    scope: SerializationCallScope,
  ) -> Result<Self, GrainCodecError> {
    let extension =
      system.extended().extension_by_type::<SerializationExtensionSharedGeneric<TB>>().ok_or_else(|| {
        GrainCodecError::ExtensionUnavailable { reason: String::from("serialization extension not installed") }
      })?;
    Ok(Self::new((*extension).clone(), scope))
  }

  fn map_error(error: &SerializationError, label: &'static str) -> GrainCodecError {
    let reason = format!("{label}: {error:?}");
    match error {
      | SerializationError::UnknownSerializer(_) | SerializationError::NotSerializable(_) => {
        GrainCodecError::SerializerNotRegistered { reason }
      },
      | SerializationError::UnknownManifest(_) | SerializationError::InvalidFormat => {
        GrainCodecError::Incompatible { reason }
      },
      | _ => match label {
        | "encode" => GrainCodecError::EncodeFailed { reason },
        | _ => GrainCodecError::DecodeFailed { reason },
      },
    }
  }
}

impl<TB: RuntimeToolbox + 'static> GrainCodec<TB> for SerializationGrainCodec<TB> {
  fn encode(
    &self,
    message: &AnyMessageGeneric<TB>,
  ) -> Result<fraktor_actor_rs::core::serialization::SerializedMessage, GrainCodecError> {
    self
      .extension
      .with_read(|ext| ext.serialize(message.payload(), self.scope))
      .map_err(|error| Self::map_error(&error, "encode"))
  }

  fn decode(
    &self,
    payload: &fraktor_actor_rs::core::serialization::SerializedMessage,
  ) -> Result<AnyMessageGeneric<TB>, GrainCodecError> {
    let decoded = self
      .extension
      .with_read(|ext| ext.deserialize(payload, None))
      .map_err(|error| Self::map_error(&error, "decode"))?;
    Ok(AnyMessageGeneric::new(decoded))
  }
}
