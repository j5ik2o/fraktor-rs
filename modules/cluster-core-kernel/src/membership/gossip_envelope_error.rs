//! Gossip envelope validation errors.

use core::fmt::{self, Formatter, Result as FmtResult};

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

impl fmt::Display for GossipEnvelopeError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::UnconfirmedIdentity { from, to } => {
        write!(f, "unconfirmed gossip envelope identities: from={from}, to={to}")
      },
    }
  }
}

impl core::error::Error for GossipEnvelopeError {}
