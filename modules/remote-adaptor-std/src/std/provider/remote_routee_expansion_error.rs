//! Error type for remote routee expansion.

use core::fmt::{Display, Formatter, Result as FmtResult};
use std::error::Error;

use fraktor_actor_core_rs::core::kernel::actor::actor_path::{ActorPath, ActorPathError};

use crate::std::provider::StdRemoteActorRefProviderError;

/// Errors returned while expanding remote router routees.
#[derive(Debug)]
pub enum RemoteRouteeExpansionError {
  /// The routee path factory failed for a routee index.
  RouteePath {
    /// Routee index being expanded.
    index:  usize,
    /// Underlying actor path construction error.
    source: ActorPathError,
  },
  /// The std provider failed to resolve a routee path.
  Provider {
    /// Routee index being expanded.
    index:  usize,
    /// Path that failed to resolve.
    path:   Box<ActorPath>,
    /// Underlying provider error.
    source: Box<StdRemoteActorRefProviderError>,
  },
}

impl RemoteRouteeExpansionError {
  pub(crate) const fn routee_path(index: usize, source: ActorPathError) -> Self {
    Self::RouteePath { index, source }
  }

  pub(crate) fn provider(index: usize, path: ActorPath, source: StdRemoteActorRefProviderError) -> Self {
    Self::Provider { index, path: Box::new(path), source: Box::new(source) }
  }
}

impl Display for RemoteRouteeExpansionError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | RemoteRouteeExpansionError::RouteePath { index, source } => {
        write!(f, "remote routee expansion failed to build routee path at index {index}: {source}")
      },
      | RemoteRouteeExpansionError::Provider { index, path, source } => {
        write!(
          f,
          "remote routee expansion failed to resolve routee at index {index} path {}: {source}",
          path.to_canonical_uri(),
        )
      },
    }
  }
}

impl Error for RemoteRouteeExpansionError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      | RemoteRouteeExpansionError::RouteePath { .. } => None,
      | RemoteRouteeExpansionError::Provider { source, .. } => Some(source.as_ref()),
    }
  }
}
