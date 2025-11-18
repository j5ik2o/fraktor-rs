//! Errors emitted by transport implementations.

use alloc::string::String;
use core::fmt;

/// Enumerates transport-specific failures.
#[derive(Debug, PartialEq, Eq)]
pub enum TransportError {
  /// Scheme was unsupported by the current build.
  UnsupportedScheme(String),
  /// Attempted to interact with an unknown authority.
  AuthorityNotBound(String),
  /// The requested channel could not be located.
  ChannelUnavailable(u64),
  /// Generic failure message.
  Io(String),
}

impl fmt::Display for TransportError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::UnsupportedScheme(scheme) => write!(f, "unsupported transport scheme: {scheme}"),
      | Self::AuthorityNotBound(authority) => write!(f, "authority not bound: {authority}"),
      | Self::ChannelUnavailable(id) => write!(f, "channel unavailable: {id}"),
      | Self::Io(message) => write!(f, "transport error: {message}"),
    }
  }
}
