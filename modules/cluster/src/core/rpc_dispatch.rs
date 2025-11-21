//! Dispatch actions produced by the RPC router.

use alloc::string::String;

use crate::core::{grain_key::GrainKey, serialized_message::SerializedMessage};

/// Outcome of a dispatch attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcDispatch {
  /// Ready to send immediately.
  Immediate {
    /// Grain key target.
    key: GrainKey,
    /// Payload to send.
    message: SerializedMessage,
    /// Absolute deadline for timeout.
    deadline: u64,
  },
  /// Enqueued due to concurrency limit.
  Queued {
    /// Queue length after enqueue.
    queue_len: usize,
  },
  /// Dropped request.
  Dropped {
    /// Reason of drop.
    reason: String,
  },
}

#[cfg(test)]
mod tests;
