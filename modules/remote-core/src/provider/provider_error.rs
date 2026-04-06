//! Failure cases produced by [`crate::provider::RemoteActorRefProvider`].

use core::fmt;

/// Failures reported by [`crate::provider::RemoteActorRefProvider`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderError {
  /// The caller passed a local `ActorPath` to a remote-only provider. The
  /// adapter layer is responsible for dispatching local traffic before ever
  /// reaching the core provider (see design Decision 3-C).
  NotRemote,
  /// The `ActorPath` is syntactically invalid for remote resolution (e.g.
  /// missing segments).
  InvalidPath,
  /// The `ActorPath` has no authority component and therefore cannot be
  /// resolved into a `UniqueAddress`.
  MissingAuthority,
  /// The `ActorPath` authority uses a transport scheme the provider does not
  /// understand.
  UnsupportedScheme,
}

impl fmt::Display for ProviderError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | ProviderError::NotRemote => f.write_str("provider: local path routed to remote-only provider"),
      | ProviderError::InvalidPath => f.write_str("provider: invalid actor path"),
      | ProviderError::MissingAuthority => f.write_str("provider: missing authority component"),
      | ProviderError::UnsupportedScheme => f.write_str("provider: unsupported path scheme"),
    }
  }
}

impl core::error::Error for ProviderError {}
