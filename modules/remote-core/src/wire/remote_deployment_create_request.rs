//! Remote deployment create request PDU payload.

use alloc::string::String;

use bytes::Bytes;

/// Request to create a deployable actor on a target node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteDeploymentCreateRequest {
  correlation_hi:     u64,
  correlation_lo:     u32,
  target_parent_path: String,
  child_name:         String,
  factory_id:         String,
  origin_node:        String,
  serializer_id:      u32,
  manifest:           Option<String>,
  payload:            Bytes,
}

impl RemoteDeploymentCreateRequest {
  /// Creates a remote deployment create request.
  #[allow(clippy::too_many_arguments)]
  #[must_use]
  pub const fn new(
    correlation_hi: u64,
    correlation_lo: u32,
    target_parent_path: String,
    child_name: String,
    factory_id: String,
    origin_node: String,
    serializer_id: u32,
    manifest: Option<String>,
    payload: Bytes,
  ) -> Self {
    Self {
      correlation_hi,
      correlation_lo,
      target_parent_path,
      child_name,
      factory_id,
      origin_node,
      serializer_id,
      manifest,
      payload,
    }
  }

  /// Returns the high 64 bits of the correlation id.
  #[must_use]
  pub const fn correlation_hi(&self) -> u64 {
    self.correlation_hi
  }

  /// Returns the low 32 bits of the correlation id.
  #[must_use]
  pub const fn correlation_lo(&self) -> u32 {
    self.correlation_lo
  }

  /// Returns the target parent actor path.
  #[must_use]
  pub fn target_parent_path(&self) -> &str {
    &self.target_parent_path
  }

  /// Returns the requested child name.
  #[must_use]
  pub fn child_name(&self) -> &str {
    &self.child_name
  }

  /// Returns the deployable factory id.
  #[must_use]
  pub fn factory_id(&self) -> &str {
    &self.factory_id
  }

  /// Returns the origin node metadata.
  #[must_use]
  pub fn origin_node(&self) -> &str {
    &self.origin_node
  }

  /// Returns the serializer id used for the deployment payload.
  #[must_use]
  pub const fn serializer_id(&self) -> u32 {
    self.serializer_id
  }

  /// Returns the optional serializer manifest.
  #[must_use]
  pub fn manifest(&self) -> Option<&str> {
    self.manifest.as_deref()
  }

  /// Returns the serialized deployment payload bytes.
  #[must_use]
  pub const fn payload(&self) -> &Bytes {
    &self.payload
  }
}
