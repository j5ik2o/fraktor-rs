//! Error types raised while resolving actor selections.

use crate::core::kernel::actor::{
  actor_path::{ActorPathError, PathResolutionError},
  actor_ref_provider::ActorRefResolveError,
  error::SendError,
};

/// Errors that can arise when resolving actor selections.
#[derive(Debug)]
pub enum ActorSelectionError {
  /// The relative path itself was invalid.
  InvalidPath(ActorPathError),
  /// Authority resolution failed (unresolved/quarantine).
  Authority(PathResolutionError),
  /// Actor reference lookup failed.
  Resolve(ActorRefResolveError),
  /// Delivery to the resolved actor failed.
  Send(SendError),
}

impl From<ActorPathError> for ActorSelectionError {
  fn from(error: ActorPathError) -> Self {
    Self::InvalidPath(error)
  }
}

impl From<PathResolutionError> for ActorSelectionError {
  fn from(error: PathResolutionError) -> Self {
    Self::Authority(error)
  }
}

impl From<SendError> for ActorSelectionError {
  fn from(error: SendError) -> Self {
    Self::Send(error)
  }
}
