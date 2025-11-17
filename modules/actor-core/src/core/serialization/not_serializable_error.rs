//! Detailed not-serializable error payload.

use alloc::string::String;

use super::{serializer_id::SerializerId, transport_information::TransportInformation};
use crate::core::actor_prim::Pid;

/// Carries context for serialization failures surfaced to the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotSerializableError {
  type_name:      String,
  serializer_id:  Option<SerializerId>,
  manifest:       Option<String>,
  pid:            Option<Pid>,
  transport_hint: Option<TransportInformation>,
}

impl NotSerializableError {
  /// Creates a new error payload with the supplied fields.
  #[must_use]
  pub fn new(
    type_name: impl Into<String>,
    serializer_id: Option<SerializerId>,
    manifest: Option<String>,
    pid: Option<Pid>,
    transport_hint: Option<TransportInformation>,
  ) -> Self {
    Self { type_name: type_name.into(), serializer_id, manifest, pid, transport_hint }
  }

  /// Returns a copy of the payload with the provided pid if absent.
  #[must_use]
  pub const fn with_pid(mut self, pid: Option<Pid>) -> Self {
    if self.pid.is_none() {
      self.pid = pid;
    }
    self
  }

  /// Returns the failing type name.
  #[must_use]
  pub fn type_name(&self) -> &str {
    &self.type_name
  }

  /// Returns a copy of the payload with the provided transport hint if absent.
  #[must_use]
  pub fn with_transport_hint(mut self, hint: Option<TransportInformation>) -> Self {
    if self.transport_hint.is_none() {
      self.transport_hint = hint;
    }
    self
  }

  /// Returns the serializer identifier that caused the failure (if known).
  #[must_use]
  pub const fn serializer_id(&self) -> Option<SerializerId> {
    self.serializer_id
  }

  /// Returns the manifest string that triggered the failure (if present).
  #[must_use]
  pub fn manifest(&self) -> Option<&str> {
    self.manifest.as_deref()
  }

  /// Returns the origin pid, if available.
  #[must_use]
  pub const fn pid(&self) -> Option<Pid> {
    self.pid
  }

  /// Returns the transport diagnostic hint (if any).
  #[must_use]
  pub const fn transport_hint(&self) -> Option<&TransportInformation> {
    self.transport_hint.as_ref()
  }
}
