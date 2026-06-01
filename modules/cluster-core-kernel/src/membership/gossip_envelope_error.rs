//! Gossip envelope validation errors.

/// Errors returned when constructing a gossip envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GossipEnvelopeError {
  /// One or both identities still carry the unconfirmed UID sentinel.
  UnconfirmedIdentity {
    /// The sender identity is unconfirmed.
    from: bool,
    /// The receiver identity is unconfirmed.
    to:   bool,
  },
}
