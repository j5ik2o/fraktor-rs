//! Errors returned by cluster request operations.

use alloc::string::String;

use crate::core::ClusterResolveError;

/// Errors raised when sending cluster requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterRequestError {
  /// Failed to resolve the target actor.
  ResolveFailed(ClusterResolveError),
  /// Failed to enqueue the request message.
  SendFailed {
    /// Failure reason.
    reason: String,
  },
  /// Failed to schedule the timeout handler.
  TimeoutScheduleFailed {
    /// Failure reason.
    reason: String,
  },
  /// Request timed out.
  Timeout,
}
