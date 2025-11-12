//! Error types for actor path construction.

use core::fmt;

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
  /// URI 全体の解析に失敗した。
  InvalidUri,
  /// サポートされていないスキームが指定された。
  UnsupportedScheme,
  /// システム名が欠落している。
  MissingSystemName,
  /// Authority の形式が不正。
  InvalidAuthority,
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
      | ActorPathError::InvalidUri => write!(f, "invalid actor path uri"),
      | ActorPathError::UnsupportedScheme => write!(f, "unsupported actor path scheme"),
      | ActorPathError::MissingSystemName => write!(f, "missing actor system name"),
      | ActorPathError::InvalidAuthority => write!(f, "invalid authority segment"),
    }
  }
}
