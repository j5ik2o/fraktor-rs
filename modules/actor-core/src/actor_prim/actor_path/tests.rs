use alloc::{vec, vec::Vec};

use super::{
  ActorPath, ActorPathComparator, ActorPathFormatter, ActorPathParser, ActorPathParts, ActorUid, GuardianKind,
  PathSegment,
};

#[test]
fn guardian_segment_is_injected_into_root() {
  let parts = ActorPathParts::local("cellsys").with_guardian(GuardianKind::System);
  let path = ActorPath::from_parts(parts);
  let segment_names: Vec<&str> = path.segments().iter().map(PathSegment::as_str).collect();
  assert_eq!(segment_names, vec!["system"]);
}

#[test]
fn root_injects_user_guardian_by_default() {
  let path = ActorPath::root();
  let segment_names: Vec<&str> = path.segments().iter().map(PathSegment::as_str).collect();
  assert_eq!(segment_names, vec!["user"]);
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

#[test]
fn comparator_ignores_uid_difference() {
  let base = ActorPath::root().child("worker");
  let with_uid = base.clone().with_uid(ActorUid::new(42));
  assert!(ActorPathComparator::eq(&base, &with_uid));
  assert_eq!(ActorPathComparator::hash(&base), ActorPathComparator::hash(&with_uid));
}

#[test]
fn parser_rejects_reserved_segment() {
  let uri = "pekko://sys/user/$system";
  assert!(ActorPathParser::parse(uri).is_err());
}

#[test]
fn parser_preserves_authority_and_guardian() {
  let uri = "pekko.tcp://cellsys@host.example.com:2552/system/logger";
  let parsed = ActorPathParser::parse(uri).expect("parse");
  assert_eq!(parsed.parts().system(), "cellsys");
  assert_eq!(parsed.parts().guardian_segment(), "system");
  assert_eq!(parsed.segments().iter().map(PathSegment::as_str).collect::<Vec<_>>(), vec!["system", "logger"]);
  assert_eq!(ActorPathFormatter::format(&parsed), uri);
}

#[test]
fn format_parse_roundtrip_samples() {
  let cases = [
    "pekko://cellsys/user/worker",
    "pekko.tcp://cellsys@127.0.0.1:2552/user/service#7",
    "pekko://cellsys/system/logger/sub",
  ];
  for uri in cases {
    let parsed = ActorPathParser::parse(uri).expect("parse");
    let formatted = ActorPathFormatter::format(&parsed);
    assert_eq!(formatted, uri);
  }
}
