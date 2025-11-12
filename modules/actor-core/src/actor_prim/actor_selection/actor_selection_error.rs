//! Error types raised while resolving actor selections.

use crate::actor_prim::actor_path::{ActorPathError, PathResolutionError};

/// Errors that can arise when resolving actor selections.
#[derive(Debug)]
pub enum ActorSelectionError {
  /// The relative path itself was invalid.
  InvalidPath(ActorPathError),
  /// Authority resolution failed (unresolved/quarantine).
  Authority(PathResolutionError),
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
