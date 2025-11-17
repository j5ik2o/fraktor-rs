//! Event payload describing serialization failures.

use alloc::{
  borrow::ToOwned,
  string::{String, ToString},
};

use super::{not_serializable_error::NotSerializableError, serializer_id::SerializerId};
use crate::core::actor_prim::Pid;

/// Event published when serialization cannot proceed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializationErrorEvent {
  type_name:      String,
  serializer_id:  Option<SerializerId>,
  manifest:       Option<String>,
  pid:            Option<Pid>,
  transport_hint: Option<String>,
}

impl SerializationErrorEvent {
  /// Creates a new event from the provided fields.
  #[must_use]
  pub fn new(
    type_name: impl Into<String>,
    serializer_id: Option<SerializerId>,
    manifest: Option<String>,
    pid: Option<Pid>,
    transport_hint: Option<String>,
  ) -> Self {
    Self { type_name: type_name.into(), serializer_id, manifest, pid, transport_hint }
  }

  /// Builds the event from a [`NotSerializableError`] instance.
  #[must_use]
  pub fn from_error(error: &NotSerializableError) -> Self {
    Self {
      type_name:      error.type_name().to_string(),
      serializer_id:  error.serializer_id(),
      manifest:       error.manifest().map(ToOwned::to_owned),
      pid:            error.pid(),
      transport_hint: error.transport_hint().and_then(|hint| hint.address().map(ToOwned::to_owned)),
    }
  }

  /// Returns the failing type name.
  #[must_use]
  pub fn type_name(&self) -> &str {
    &self.type_name
  }

  /// Returns the serializer identifier if known.
  #[must_use]
  pub const fn serializer_id(&self) -> Option<SerializerId> {
    self.serializer_id
  }

  /// Returns the manifest string if provided.
  #[must_use]
  pub fn manifest(&self) -> Option<&str> {
    self.manifest.as_deref()
  }

  /// Returns the origin pid if available.
  #[must_use]
  pub const fn pid(&self) -> Option<Pid> {
    self.pid
  }

  /// Returns the transport hint (address) if available.
  #[must_use]
  pub fn transport_hint(&self) -> Option<&str> {
    self.transport_hint.as_deref()
  }
}
