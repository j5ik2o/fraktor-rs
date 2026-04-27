//! Discriminator for non-handshake-able association states.

use core::fmt::{Display, Formatter, Result as FmtResult};

/// Identifies which non-handshake-able state caused the association to reject
/// an inbound handshake.
///
/// Stored inside [`super::HandshakeValidationError::RejectedInState`] so that
/// renames of [`super::AssociationState`] variants surface as compile errors
/// instead of silent string drift.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandshakeRejectedState {
  /// `AssociationState::Idle` — no handshake in flight.
  Idle,
  /// `AssociationState::Gated` — temporary gate is active.
  Gated,
  /// `AssociationState::Quarantined` — peer has been quarantined.
  Quarantined,
}

impl HandshakeRejectedState {
  /// Returns a stable identifier suitable for logs and error messages.
  #[must_use]
  pub const fn as_str(&self) -> &'static str {
    match self {
      | Self::Idle => "Idle",
      | Self::Gated => "Gated",
      | Self::Quarantined => "Quarantined",
    }
  }
}

impl Display for HandshakeRejectedState {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.write_str(self.as_str())
  }
}
