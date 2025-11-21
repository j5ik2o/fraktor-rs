//! Events emitted from identity resolution.

use alloc::string::String;

use crate::core::membership_version::MembershipVersion;

/// Events emitted from identity resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityEvent {
  /// Resolution succeeded using the latest membership version.
  ResolvedLatest {
    /// Target authority.
    authority: String,
    /// Membership version used.
    version: MembershipVersion,
  },
  /// Resolution was blocked by quarantine.
  Quarantined {
    /// Target authority.
    authority: String,
    /// Quarantine reason.
    reason: String,
    /// Current membership version.
    version: MembershipVersion,
  },
  /// Authority was not present or not reachable.
  UnknownAuthority {
    /// Target authority.
    authority: String,
    /// Current membership version.
    version: MembershipVersion,
  },
  /// Resolve request was rejected due to invalid format.
  InvalidFormat {
    /// Failure reason.
    reason: String,
  },
}
