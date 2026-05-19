//! Guardian kind enumeration.
//!
//! This module contains the guardian kind enumeration.

/// Identifies which guardian slot was affected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardianKind {
  /// Root guardian at `/`.
  Root,
  /// System guardian at `/system`.
  System,
  /// User guardian at `/user`.
  User,
}
