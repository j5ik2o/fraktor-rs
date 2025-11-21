//! Membership error types.

use alloc::string::String;

/// Errors that can occur while mutating the membership table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MembershipError {
  /// Another node already owns the authority.
  AuthorityConflict {
    /// Authority in conflict.
    authority: String,
    /// Existing node id bound to the authority.
    existing_node_id: String,
    /// Requested node id that caused the collision.
    requested_node_id: String,
  },
  /// Target authority was not found in the table.
  UnknownAuthority {
    /// Authority string.
    authority: String,
  },
}
