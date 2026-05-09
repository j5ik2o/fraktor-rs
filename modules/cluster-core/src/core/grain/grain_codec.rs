//! Codec abstraction for grain messages.

use fraktor_actor_core_kernel_rs::{actor::messaging::AnyMessage, serialization::SerializedMessage};

use super::GrainCodecError;

/// Codec used by the grain API.
pub trait GrainCodec: Send + Sync {
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
