//! Errors that can be returned by the RPC router.

use alloc::string::String;

/// RPC failure reasons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcError {
  /// Schema negotiation failed.
  SchemaMismatch {
    /// Negotiated version (if any).
    negotiated: Option<u32>,
    /// Version carried by the message.
    message_version: u32,
  },
  /// Serialization/validation failed.
  SerializationFailed {
    /// Human-readable error.
    reason: String,
  },
}

#[cfg(test)]
mod tests;
