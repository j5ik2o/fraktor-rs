//! Errors returned by `ActorSystem::resolve_actor_ref`.

use alloc::string::String;
use core::fmt::{Display, Formatter, Result as FmtResult};

#[cfg(test)]
#[path = "actor_ref_resolve_error_test.rs"]
mod tests;

/// Resolution failures for actor references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorRefResolveError {
  /// The provided actor path scheme is not supported.
  UnsupportedScheme,
  /// No provider is registered for the requested scheme.
  ProviderMissing,
  /// System has not completed bootstrap.
  SystemNotBootstrapped,
  /// Authority information is incomplete or unavailable.
  InvalidAuthority,
  /// Provider failed to resolve the path.
  NotFound(String),
}

impl Display for ActorRefResolveError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::UnsupportedScheme => write!(f, "unsupported actor path scheme"),
      | Self::ProviderMissing => write!(f, "no actor-ref provider registered for scheme"),
      | Self::SystemNotBootstrapped => write!(f, "actor system not bootstrapped yet"),
      | Self::InvalidAuthority => write!(f, "authority is missing or incomplete"),
      | Self::NotFound(reason) => write!(f, "actor reference could not be resolved: {reason}"),
    }
  }
}
