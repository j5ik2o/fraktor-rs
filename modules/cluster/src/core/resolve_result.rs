//! PID resolution status.

use alloc::string::String;

use fraktor_actor_rs::core::actor_prim::actor_path::ActorPath;

use crate::core::membership_version::MembershipVersion;

/// Result of PID resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveResult {
  /// Resolution succeeded and returns canonical ActorPath.
  Ready {
    /// Resolved path.
    actor_path: ActorPath,
    /// Membership version used.
    version:    MembershipVersion,
  },
  /// Authority is removed/unreachable or missing.
  Unreachable {
    /// Authority string.
    authority: String,
    /// Membership version observed.
    version:   MembershipVersion,
  },
  /// Authority is quarantined.
  Quarantine {
    /// Authority string.
    authority: String,
    /// Quarantine reason.
    reason:    String,
    /// Membership version observed.
    version:   MembershipVersion,
  },
}
