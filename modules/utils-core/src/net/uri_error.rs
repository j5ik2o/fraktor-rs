//! URI parsing errors.

#[cfg(test)]
#[path = "uri_error_test.rs"]
mod tests;

use core::fmt::{Display, Formatter, Result as FmtResult};

/// Errors that may occur during URI parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UriError {
  /// Invalid scheme format.
  InvalidScheme,
  /// Invalid authority format.
  InvalidAuthority,
  /// Invalid path format.
  InvalidPath,
  /// Invalid query format.
  InvalidQuery,
  /// Invalid fragment format.
  InvalidFragment,
  /// Invalid percent encoding.
  InvalidPercentEncoding,
  /// Unexpected end of input.
  UnexpectedEof,
}

impl Display for UriError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | UriError::InvalidScheme => write!(f, "invalid scheme format"),
      | UriError::InvalidAuthority => write!(f, "invalid authority format"),
      | UriError::InvalidPath => write!(f, "invalid path format"),
      | UriError::InvalidQuery => write!(f, "invalid query format"),
      | UriError::InvalidFragment => write!(f, "invalid fragment format"),
      | UriError::InvalidPercentEncoding => write!(f, "invalid percent encoding"),
      | UriError::UnexpectedEof => write!(f, "unexpected end of input"),
    }
  }
}
