//! Relative path resolution for ActorSelection.

use alloc::{string::ToString, vec::Vec};

use crate::actor_prim::actor_path::{ActorPath, ActorPathError, PathSegment};

/// Resolves relative actor selection expressions against a base path.
pub struct ActorSelectionResolver;

impl ActorSelectionResolver {
  /// Resolves a relative path against a base ActorPath.
  ///
  /// Supports:
  /// - `.` (current)
  /// - `..` (parent, fails if escaping guardian root)
  /// - child names
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
}
