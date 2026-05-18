//! Path resolution error types.

use core::fmt::{Display, Formatter, Result as FmtResult};

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
  /// Authority is unresolved and its deferred queue cannot accept more messages.
  AuthorityDeferredQueueFull,
  /// UID is reserved and cannot be reused yet.
  UidReserved {
    /// The reserved UID.
    uid: ActorUid,
  },
}

impl Display for PathResolutionError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | PathResolutionError::PidUnknown => write!(f, "PID not found in registry"),
      | PathResolutionError::AuthorityUnresolved => write!(f, "authority is not resolved"),
      | PathResolutionError::AuthorityQuarantined => write!(f, "authority is quarantined"),
      | PathResolutionError::AuthorityDeferredQueueFull => write!(f, "authority deferred queue is full"),
      | PathResolutionError::UidReserved { uid } => {
        write!(f, "UID {} is reserved and cannot be reused", uid.value())
      },
    }
  }
}
