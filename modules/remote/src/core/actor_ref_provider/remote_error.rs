//! Error types returned by the remote actor-ref provider.

use alloc::string::{String, ToString};

/// Describes failures that occur while constructing remote actor references.
#[derive(Debug)]
pub enum RemoteActorRefProviderError {
  /// The actor path did not include a remote authority segment.
  MissingAuthority,
  /// The authority segment could not be parsed into host/port.
  InvalidAuthority(String),
  /// Remoting control rejected the association request.
  Remoting(String),
  /// The actor system has been dropped and is no longer available.
  SystemUnavailable,
}

impl core::fmt::Display for RemoteActorRefProviderError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      | Self::MissingAuthority => write!(f, "actor path missing remote authority"),
      | Self::InvalidAuthority(authority) => write!(f, "invalid authority format: {authority}"),
      | Self::Remoting(message) => write!(f, "remoting error: {message}"),
      | Self::SystemUnavailable => write!(f, "actor system has been dropped"),
    }
  }
}

impl From<crate::core::remoting_extension::RemotingError> for RemoteActorRefProviderError {
  fn from(value: crate::core::remoting_extension::RemotingError) -> Self {
    Self::Remoting(value.to_string())
  }
}
