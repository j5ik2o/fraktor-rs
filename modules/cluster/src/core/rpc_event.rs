//! Events emitted from RPC router operations.

use alloc::string::String;

use crate::core::grain_key::GrainKey;

/// Observable RPC events for metrics and debugging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcEvent {
  /// Request dispatched immediately.
  Dispatched {
    /// Target grain key.
    key: GrainKey,
    /// Absolute deadline when it times out.
    deadline: u64,
  },
  /// Request queued due to concurrency limit.
  Queued {
    /// Target grain key.
    key: GrainKey,
    /// Queue length after enqueue.
    queue_len: usize,
  },
  /// Oldest request was dropped.
  DroppedOldest {
    /// Target grain key.
    key: GrainKey,
    /// Drop reason.
    reason: String,
  },
  /// Request was rejected because queue was full.
  Rejected {
    /// Target grain key.
    key: GrainKey,
    /// Reason for rejection.
    reason: String,
  },
  /// Cached request was promoted after completion.
  Promoted {
    /// Target grain key.
    key: GrainKey,
  },
  /// Request timed out.
  TimedOut {
    /// Target grain key.
    key: GrainKey,
  },
  /// Serialization failed before dispatch.
  SerializationFailed {
    /// Target grain key.
    key: GrainKey,
    /// Validation reason.
    reason: String,
  },
  /// Schema mismatch detected.
  SchemaMismatch {
    /// Target grain key.
    key: GrainKey,
    /// Version found in the message.
    message_version: u32,
  },
}

#[cfg(test)]
mod tests;
