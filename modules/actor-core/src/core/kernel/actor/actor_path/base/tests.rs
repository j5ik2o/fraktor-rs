use alloc::{format, vec, vec::Vec};

use super::ActorPath;
use crate::core::kernel::actor::actor_path::{ActorPathError, ActorPathParts, GuardianKind, PathSegment};

#[test]
fn try_from_segments_validates_and_injects_user_guardian() {
  let path = ActorPath::try_from_segments(["service", "worker"]).expect("path");

  let segments: Vec<&str> = path.segments().iter().map(PathSegment::as_str).collect();
  assert_eq!(segments, vec!["user", "service", "worker"]);
}

#[test]
fn try_from_segments_rejects_invalid_segment() {
  let result = ActorPath::try_from_segments(["service", "$system"]);

  assert!(matches!(result, Err(ActorPathError::ReservedSegment)));
}

#[test]
fn root_relative_string_is_user_when_guardian_is_user() {
  let parts = ActorPathParts::local("cellsys").with_guardian(GuardianKind::User);
  let path = ActorPath::from_parts_and_segments(parts, Vec::new(), None);

  assert_eq!(path.to_relative_string(), "/user");
}

#[test]
fn debug_uses_relative_path() {
  let path = ActorPath::root().child("worker");

  assert_eq!(format!("{path:?}"), "ActorPath(\"/user/worker\")");
}
