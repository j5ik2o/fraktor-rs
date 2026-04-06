//! Errors raised during grain calls.

use super::GrainCodecError;
use crate::core::{ClusterRequestError, ClusterResolveError};

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
