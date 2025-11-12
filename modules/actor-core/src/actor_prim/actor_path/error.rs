//! Error types produced while constructing actor paths.

use core::fmt;

use super::ActorUid;

/// Errors that can occur while constructing or formatting actor paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorPathError {
  /// Provided segment was empty.
  EmptySegment,
  /// Segment started with a reserved `$` prefix.
  ReservedSegment,
  /// Segment contained a character outside the RFC2396 whitelist.
  InvalidSegmentChar {
    /// Offending character.
    ch:    char,
    /// Character index in the original string.
    index: usize,
  },
  /// Percent encoding was malformed.
  InvalidPercentEncoding,
  /// Relative path escaped beyond guardian root.
  RelativeEscape,
}

impl fmt::Display for ActorPathError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | ActorPathError::EmptySegment => write!(f, "path segment must not be empty"),
      | ActorPathError::ReservedSegment => write!(f, "path segment must not start with '$'"),
      | ActorPathError::InvalidSegmentChar { ch, index } => {
        write!(f, "invalid character '{ch}' at position {index}")
      },
      | ActorPathError::InvalidPercentEncoding => write!(f, "invalid percent encoding sequence"),
      | ActorPathError::RelativeEscape => write!(f, "relative path escapes beyond guardian root"),
    }
  }
}

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
