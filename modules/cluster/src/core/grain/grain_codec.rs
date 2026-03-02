//! Codec abstraction for grain messages.

use fraktor_actor_rs::core::{messaging::AnyMessage, serialization::SerializedMessage};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::GrainCodecError;

/// Codec used by the grain API.
pub trait GrainCodec<TB: RuntimeToolbox + 'static>: Send + Sync {
  /// Encodes a message payload.
  ///
  /// # Errors
  ///
  /// Returns an error if the payload cannot be serialized by the codec.
  fn encode(&self, message: &AnyMessage) -> Result<SerializedMessage, GrainCodecError>;

  /// Decodes a serialized payload.
  ///
  /// # Errors
  ///
  /// Returns an error if the payload cannot be deserialized or is incompatible.
  fn decode(&self, payload: &SerializedMessage) -> Result<AnyMessage, GrainCodecError>;
}
