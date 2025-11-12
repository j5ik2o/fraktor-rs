//! Path resolution error types.

use core::fmt;

use super::ActorUid;

/// Errors that can occur during path resolution in the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathResolutionError {
  /// The requested PID was not found in the registry.
  PidUnknown,
  /// Authority could not be resolved (no mapping exists).
  AuthorityUnresolved,
  /// Authority is currently quarantined.
  AuthorityQuarantined,
  /// UID is reserved and cannot be reused yet.
  UidReserved {
    /// The reserved UID.
    uid: ActorUid,
  },
}

impl fmt::Display for PathResolutionError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | PathResolutionError::PidUnknown => write!(f, "PID not found in registry"),
      | PathResolutionError::AuthorityUnresolved => write!(f, "authority is not resolved"),
      | PathResolutionError::AuthorityQuarantined => write!(f, "authority is quarantined"),
      | PathResolutionError::UidReserved { uid } => {
        write!(f, "UID {} is reserved and cannot be reused", uid.value())
      },
    }
  }
}
