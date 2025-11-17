//! Relative path resolution for ActorSelection.

use alloc::{string::ToString, vec::Vec};

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::actor_selection_error::ActorSelectionError;
use crate::core::{
  actor_prim::actor_path::{ActorPath, ActorPathError, PathResolutionError, PathSegment},
  messaging::AnyMessageGeneric,
  system::{AuthorityState, RemoteAuthorityManagerGeneric},
};

/// Resolves relative actor selection expressions against a base path.
pub struct ActorSelectionResolver;

impl ActorSelectionResolver {
  /// Resolves a relative path against a base ActorPath.
  ///
  /// Supports:
  /// - `.` (current)
  /// - `..` (parent, fails if escaping guardian root)
  /// - child names
  ///
  /// # Errors
  ///
  /// Returns [`ActorPathError::RelativeEscape`] if `..` attempts to escape the guardian root.
  pub fn resolve_relative(base: &ActorPath, selection: &str) -> Result<ActorPath, ActorPathError> {
    if selection.is_empty() {
      return Ok(base.clone());
    }

    let parts: Vec<&str> = selection.split('/').filter(|s| !s.is_empty()).collect();
    let mut segments: Vec<PathSegment> = base.segments().to_vec();

    for part in parts {
      match part {
        | "." => {
          // 現在のパスを維持
        },
        | ".." => {
          if segments.len() <= 1 {
            // guardian より上位へ遡ることは禁止
            return Err(ActorPathError::RelativeEscape);
          }
          segments.pop();
        },
        | name => {
          segments.push(PathSegment::new(name.to_string())?);
        },
      }
    }

    Ok(ActorPath::from_parts_and_segments(base.parts().clone(), segments, None))
  }

  /// Ensures that the remote authority referenced by `path` is in a sendable state.
  ///
  /// # Errors
  ///
  /// If the authority is unresolved, the provided `message` is deferred (when present) and
  /// [`PathResolutionError::AuthorityUnresolved`] is returned. When the authority is quarantined,
  /// [`PathResolutionError::AuthorityQuarantined`] is returned immediately.
  pub fn ensure_authority_state<TB: RuntimeToolbox + 'static>(
    path: &ActorPath,
    authority_manager: &RemoteAuthorityManagerGeneric<TB>,
    message: Option<AnyMessageGeneric<TB>>,
  ) -> Result<(), PathResolutionError> {
    let Some(authority) = path.parts().authority() else {
      return Ok(());
    };
    let endpoint = authority.endpoint();
    match authority_manager.state(&endpoint) {
      | AuthorityState::Connected => Ok(()),
      | AuthorityState::Unresolved => {
        if let Some(envelope) = message {
          authority_manager
            .defer_send(endpoint.clone(), envelope)
            .map_err(|_| PathResolutionError::AuthorityQuarantined)?;
        }
        Err(PathResolutionError::AuthorityUnresolved)
      },
      | AuthorityState::Quarantine { .. } => Err(PathResolutionError::AuthorityQuarantined),
    }
  }

  /// Resolves a relative path and validates the remote authority state (if present).
  ///
  /// # Errors
  ///
  /// Returns [`ActorSelectionError`] when either the relative path is invalid or the authority
  /// remains unresolved/quarantined.
  pub fn resolve_relative_with_authority<TB: RuntimeToolbox + 'static>(
    base: &ActorPath,
    selection: &str,
    authority_manager: &RemoteAuthorityManagerGeneric<TB>,
    message: Option<AnyMessageGeneric<TB>>,
  ) -> Result<ActorPath, ActorSelectionError> {
    let resolved = Self::resolve_relative(base, selection)?;
    Self::ensure_authority_state(&resolved, authority_manager, message).map_err(ActorSelectionError::from)?;
    Ok(resolved)
  }
}
