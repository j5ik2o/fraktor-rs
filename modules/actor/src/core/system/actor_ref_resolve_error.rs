//! Errors returned by `ActorSystem::resolve_actor_ref`.

use alloc::string::String;

/// Resolution failures for actor references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorRefResolveError {
  /// The provided actor path scheme is not supported.
  UnsupportedScheme,
  /// No provider is registered for the requested scheme.
  ProviderMissing,
  /// Authority information is incomplete or unavailable.
  InvalidAuthority,
  /// Provider failed to resolve the path.
  NotFound(String),
}

impl core::fmt::Display for ActorRefResolveError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      | Self::UnsupportedScheme => write!(f, "unsupported actor path scheme"),
      | Self::ProviderMissing => write!(f, "no actor-ref provider registered for scheme"),
      | Self::InvalidAuthority => write!(f, "authority is missing or incomplete"),
      | Self::NotFound(reason) => write!(f, "actor reference could not be resolved: {reason}"),
    }
  }
}
