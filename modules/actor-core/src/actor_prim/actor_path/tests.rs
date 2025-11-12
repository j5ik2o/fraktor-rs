use alloc::{vec, vec::Vec};

use super::{ActorPath, ActorPathFormatter, ActorPathParts, GuardianKind, PathSegment};

#[test]
fn guardian_segment_is_injected_into_root() {
  let parts = ActorPathParts::local("cellsys").with_guardian(GuardianKind::System);
  let path = ActorPath::from_parts(parts);
  let segment_names: Vec<&str> = path.segments().iter().map(PathSegment::as_str).collect();
  assert_eq!(segment_names, vec!["system"]);
}

#[test]
fn path_segment_rejects_reserved_dollar_prefix() {
  let result = PathSegment::new("$user");
  assert!(result.is_err());
}

#[test]
fn canonical_uri_includes_scheme_system_and_segments() {
  let parts = ActorPathParts::local("cellsys")
    .with_guardian(GuardianKind::User)
    .with_authority_host("host.example.com".into())
    .with_authority_port(2552);
  let path = ActorPath::from_parts(parts).child("service").child("worker");
  let canonical = ActorPathFormatter::format(&path);
  assert_eq!(canonical, "pekko://cellsys@host.example.com:2552/user/service/worker");
}
