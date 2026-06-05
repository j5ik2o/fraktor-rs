//! Cluster wire decode failure taxonomy.

/// Typed failures raised while decoding cluster wire messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterWireDecodeFailure {
  /// The frame declares an unsupported version.
  UnknownVersion,
  /// The frame declares an unknown payload kind tag.
  UnknownPayloadKind,
  /// The actor-core manifest route is unknown.
  UnknownManifest,
  /// The frame bytes are malformed.
  MalformedPayload,
}
