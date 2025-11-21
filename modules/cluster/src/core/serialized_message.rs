//! Serialized message envelope used by RPC router.

use alloc::vec::Vec;

/// RPC payload with schema version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializedMessage {
  /// Serialized bytes.
  pub bytes: Vec<u8>,
  /// Schema version of the payload.
  pub schema_version: u32,
}

impl SerializedMessage {
  /// Creates a new serialized message.
  pub fn new(bytes: Vec<u8>, schema_version: u32) -> Self {
    Self { bytes, schema_version }
  }

  /// Returns true when the payload is empty.
  pub fn is_empty(&self) -> bool {
    self.bytes.is_empty()
  }
}

#[cfg(test)]
mod tests;
