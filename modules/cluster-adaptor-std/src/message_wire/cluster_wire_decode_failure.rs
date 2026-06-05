//! Cluster wire decode failure taxonomy.

use thiserror::Error;

/// Typed failures raised while decoding cluster wire messages.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClusterWireDecodeFailure {
  /// The frame declares an unsupported version.
  #[error("unsupported cluster wire frame version")]
  UnknownVersion,
  /// The frame declares an unknown payload kind tag.
  #[error("unknown cluster wire payload kind")]
  UnknownPayloadKind,
  /// The frame bytes are malformed.
  #[error("malformed cluster wire frame payload")]
  MalformedPayload,
}
