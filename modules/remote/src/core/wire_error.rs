//! Errors emitted when encoding or decoding remoting wire frames.

use alloc::string::FromUtf8Error;

use fraktor_actor_rs::core::{actor_prim::actor_path::ActorPathError, serialization::SerializationError};

/// Represents failures while decoding transport frames.
#[derive(Debug)]
pub enum WireError {
  /// The provided payload does not follow the expected binary layout.
  InvalidFormat,
  /// Actor path parsing failed while reconstructing the envelope.
  InvalidActorPath(ActorPathError),
  /// Serialized payload decoding failed.
  Serialization(SerializationError),
  /// UTF-8 decoding failed for textual fields.
  Utf8Error,
}

impl From<SerializationError> for WireError {
  fn from(error: SerializationError) -> Self {
    Self::Serialization(error)
  }
}

impl From<ActorPathError> for WireError {
  fn from(error: ActorPathError) -> Self {
    Self::InvalidActorPath(error)
  }
}

impl From<FromUtf8Error> for WireError {
  fn from(_: FromUtf8Error) -> Self {
    Self::Utf8Error
  }
}
