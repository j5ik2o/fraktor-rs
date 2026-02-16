//! Errors raised while constructing cluster identities.

/// Error conditions returned by [`ClusterIdentity`](super::cluster_identity::ClusterIdentity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterIdentityError {
  /// The kind component is empty.
  EmptyKind,
  /// The identity component is empty.
  EmptyIdentity,
}
