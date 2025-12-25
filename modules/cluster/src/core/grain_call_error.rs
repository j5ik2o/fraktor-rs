//! Errors raised during grain calls.

use crate::core::{ClusterRequestError, ClusterResolveError, GrainCodecError};

/// Errors raised while invoking grains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrainCallError {
  /// Failed to resolve the target grain.
  ResolveFailed(ClusterResolveError),
  /// Failed to send the request.
  RequestFailed(ClusterRequestError),
  /// Failed to encode or decode messages.
  CodecFailed(GrainCodecError),
}
